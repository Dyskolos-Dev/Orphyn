# Jellyfin data layer

The only way the app talks to Jellyfin. Screens import from `@renderer/jellyfin` and never use raw
HTTP. Everything is scoped to the **active server**; switching server clears the query cache.

```ts
import { useUserViews, useItems, flattenPages, imageUrl, getPlaybackInfo } from '@renderer/jellyfin'
```

## Setup (once, in `App.tsx`)

Wrap the app in `<JellyfinProvider>`. It provides the TanStack `QueryClient` and calls
`useSession.getState().init()` (loads accounts from main and activates the stored active one).

```tsx
<JellyfinProvider>
  <RouterProvider router={router} />
</JellyfinProvider>
```

`useSession` (zustand) exposes `status` (`idle | loading | ready | no-server | needs-login`),
`accounts`, `account` (active), `api` (SDK `Api` for the active server), `needsLogin`, and actions
`switchServer(id)`, `activate(account)`, `signOut(id)`, `refreshAccounts()`. A 401 on the active server
sets `status: 'needs-login'`; route to the login screen then.

## Accounts (main process, `window.orphyn.accounts`)

`list, add, rename, remove, setActive, getActive, getToken, getDeviceId, getStatus`.

- Stored in `accounts.json` in the app data folder; the access token itself is in the OS keychain
  (`accessToken` is always `null` in the file).
- **Passwords are never stored or logged.** Only the token returned by `AuthenticateByName` is kept.
- If encryption is unavailable (Linux without keyring, or the `basic_text` backend) tokens are kept
  **in memory only**, a warning is logged and `getStatus()` returns `{ encrypted: false, warning }`:
  show it in settings. After a restart such accounts have `hasToken: false` and need a new login.
- The renderer only sees `ServerAccount` (no token). `getToken(id)` returns it when the Api is built.
- `deviceId` is a stable uuid per install, sent as the Jellyfin `DeviceId`.

## Connect flow (onboarding / settings screens)

```ts
const probe = await probeServer('nas') // normalises: scheme, :8096, /jellyfin, trailing slash
if (!probe.ok) show(probe.error.kind) // invalid_url | unreachable | not_jellyfin | tls | version_too_old
const res = await connectServer({ url: 'nas', username, password, name: 'Salon' })
if (res.ok) await useSession.getState().activate(res.account)
else show(res.error.kind) // probe errors, or invalid_credentials | unreachable | server_error
await useSession.getState().signOut(account.id) // revokes the session on the server, then removes
```

`login(url, user, password, deviceId)` is available on its own. Client identity is `Orphyn` + app version.
Servers older than 10.9 are refused (`version_too_old`).

## Query hooks

All return a TanStack query result and are disabled until a session exists.

| Hook                                                                                   | Returns                                                                                                                                                                                                        |
| -------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `useUserViews()`                                                                       | libraries (`CollectionType`: movies, tvshows, music...)                                                                                                                                                        |
| `useItems(params)`                                                                     | infinite pages of `GET /Items` (see `ItemsParams`)                                                                                                                                                             |
| `useItem(id)`                                                                          | one item with details                                                                                                                                                                                          |
| `useResumeItems('video' \| 'audio', limit?)`                                           | continue watching / listening                                                                                                                                                                                  |
| `useNextUp(parentId?, limit?)`                                                         | next episodes                                                                                                                                                                                                  |
| `useLatest(parentId, limit?)`                                                          | recently added                                                                                                                                                                                                 |
| `useSeasons(seriesId)`, `useEpisodes(seriesId, seasonId?)`                             | TV structure                                                                                                                                                                                                   |
| `useSimilar(id)`                                                                       | similar items                                                                                                                                                                                                  |
| `useAlbums / useArtists / useAlbumArtists / usePlaylists / useSongs(params)`           | infinite lists                                                                                                                                                                                                 |
| `useArtistAlbums(artistId)`, `useAlbumTracks(albumId)`, `usePlaylistItems(playlistId)` | music detail                                                                                                                                                                                                   |
| `useInstantMix(itemId, limit?)`                                                        | instant mix                                                                                                                                                                                                    |
| `useLyrics(itemId)`                                                                    | `null` or `{ synced, lines: [{ text, startMs }] }` (Jellyfin 10.9 lyrics API)                                                                                                                                  |
| `useSearch(term)`                                                                      | `{ movies, series, episodes, albums, artists, tracks }` (enabled from 2 chars)                                                                                                                                 |
| `useMarkPlayed()`                                                                      | `mutate({ id, value: true \| false })` played / unplayed                                                                                                                                                       |
| `useToggleFavorite()`                                                                  | `mutate({ id, value })`; flips the heart in every cached list / detail at once, rolls back on error                                                                                                            |
| `useUserPlaylists()`                                                                   | the user's editable music playlists (`/Playlists/{id}/Users/{userId}` → `CanEdit`)                                                                                                                             |
| `useFavoriteTracks / useFavoriteAlbums / useFavoriteArtists()`                         | all favourites of one kind                                                                                                                                                                                     |
| `usePlaylistActions()`                                                                 | `create(name, ids?, {isPublic?, description?})`, `rename`, `remove`, `add(playlistId, ids)`, `removeEntries(playlistId, entryIds)` (returns what Undo needs), `restore`, `move(playlistId, entryId, newIndex)` |
| `useIsFavorite(id)`                                                                    | heart state of an item as the caches know it (for the player UI)                                                                                                                                               |

Playlist API (Jellyfin 10.11 checked): `POST /Playlists` (`Ids` may hold albums, the server expands them), `POST /Playlists/{id}` (rename, ≥ 10.9), `DELETE /Items/{id}`, `POST /Playlists/{id}/Items?ids=`, `DELETE /Playlists/{id}/Items?entryIds=` (PlaylistItemId, one per occurrence), `POST /Playlists/{id}/Items/{entryId}/Move/{newIndex}` (final 0-based position). A playlist description is stored with `POST /Items/{id}` (`Overview`), which only administrators may call: for others it is silently not saved.

Infinite hooks:

```tsx
const q = useAlbums({ sortBy: ['SortName'], pageSize: 60 })
const albums = flattenPages(q.data)
// <Grid onEndReached={() => q.hasNextPage && q.fetchNextPage()} />
```

Keys are `['jf', accountId, ...]` (`jfKeys`), so two servers never share cache entries; mutations
invalidate `jfKeys.all(accountId)`. Stale times: libraries 10 min, catalog 5 min, resume/next-up 15 s,
lyrics 1 h.

## Images

```ts
imageUrl(item, 'Primary', { maxWidth: 300, quality: 80 }) // Primary | Backdrop | Logo | Thumb
```

Fallbacks: episodes/seasons use the series poster, tracks use the album cover, backdrops/logos/thumbs use
the parent's. Returns `undefined` when there is no image (render a placeholder). `buildImageUrl(baseUrl, ...)`
is the pure variant.

## Playback

```ts
const info = await getPlaybackInfo(itemId, {
  startTimeTicks: secondsToTicks(resumeSeconds),
  audioStreamIndex,
  subtitleStreamIndex,
  maxBitrate, // all optional
})
// info: { url, isHls, method, playSessionId, mediaSource, mediaSourceId, tracks, ... }
```

- `isHls` → attach with **hls.js**; otherwise set `video.src = info.url`.
- `method`: `DirectPlay | DirectStream | Transcode`. Chromium direct-plays mp4/webm and common audio;
  everything else becomes HLS fMP4 h264/aac. The profile detects HEVC/AV1/VP9 support at runtime
  (`buildDeviceProfile`, `detectCodecSupport`).
- `tracks.audio` / `tracks.subtitles`: use `index` for switching (call `getPlaybackInfo` again with the new
  index and resume at the current position). Text subtitles have `vttUrl` for a `<track>` element; image
  subtitles (PGS/DVD) have `burnInRequired: true` and need `subtitleStreamIndex`.
- Music: `getPlaybackInfo(trackId, { kind: 'audio' })`, or `universalAudioUrl(api, userId, id)`; plain
  `<audio src>` works (direct play, else progressive MP3).
- URLs carry `ApiKey` because media elements cannot send headers. Do not log or persist them.
- Throws `PlaybackError` when the server offers no stream.

### Media segments and trailers

- `getPlaybackInfo()` (video) also returns `segments`: `MediaSegment[]` (`Intro`, `Outro`, `Recap`, `Preview`, `Commercial`; in seconds, sorted) read from `GET /MediaSegments/{id}` (Jellyfin 10.10+). A server without the endpoint, or any error, gives `[]`: nothing throws. `segmentAt(segments, seconds, types, tail?)` finds the segment at a given time.
- `useLocalTrailers(id, item.LocalTrailerCount)`: trailer files of a movie or series (`GET /Items/{id}/LocalTrailers`); no request when the count is 0. They play in the video player as `/video/play/{trailerId}?trailer=1` (no resume, nothing reported). `item.RemoteTrailers` holds the web links (usually YouTube).

### Reporting

```ts
const reporter = new PlaybackReporter(targetFromInfo(info), () => ({
  positionTicks: secondsToTicks(video.currentTime),
  isPaused: video.paused,
}))
await reporter.start() // POST /Sessions/Playing, then progress every 10 s
video.onpause = () => void reporter.pause()
video.onplay = () => void reporter.resume()
video.onseeked = () => void reporter.tick()
await reporter.stop() // on end / unmount: POST /Sessions/Playing/Stopped
```

Low-level: `reportPlaybackStart / reportPlaybackProgress / reportPlaying / reportPlaybackStopped`.
The reporter captures the Api when created, so a server switch cannot send reports to the wrong server,
and errors go to the optional `onError` instead of throwing into the player.

### Casting

`streamUrlForCast(itemId, 'audio' | 'video', { transcode?: boolean, maxBitrate? })` returns an absolute URL
with `ApiKey` for Chromecast/UPnP. Use `transcode: true` for video to get an HLS h264/aac stream. The
renderer must reach the server at the address saved for the account (not `localhost`).

## Checked against a real server

`src/renderer/src/jellyfin/live.test.ts` (opt-in) probes, logs in, lists libraries, exercises resume /
next-up / latest / albums / artists / search / lyrics, and resolves + range-fetches a movie URL and a track
URL. Run it with:

```
ORPHYN_LIVE_URL=https://demo.jellyfin.org/stable ORPHYN_LIVE_USER=demo npx vitest run src/renderer/src/jellyfin/live.test.ts
```

It passed against the public demo (Jellyfin 12.4.0), which also showed that newer servers reject the
legacy `api_key` query name on some routes, hence `ApiKey`.
