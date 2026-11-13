# QA: end-to-end testing against a real Jellyfin server

Jellyra has an end-to-end suite that drives the **built** Electron app (`out/main/index.js`) with
Playwright's Electron driver against a real Jellyfin server. It is separate from `npm run check`
(which stays offline) and skips itself when no server is configured.

## Running it

```sh
# PowerShell
$env:JELLYRA_E2E_URL = "https://jellyfin.example.com"
$env:JELLYRA_E2E_USER = "user"
$env:JELLYRA_E2E_PASSWORD = "password"
npm run e2e

# POSIX shells
JELLYRA_E2E_URL=https://jellyfin.example.com JELLYRA_E2E_USER=user JELLYRA_E2E_PASSWORD=password npm run e2e
```

- `npm run e2e` builds the app, then runs `playwright test` (config: `playwright.config.ts`, specs
  in `e2e/`). Without the three variables every scenario is reported as skipped and the run passes.
- Never commit credentials: they are only read from the environment.
- Each run uses a fresh temporary user data directory (created by `e2e/global-setup.ts`, passed to
  the app through `JELLYRA_USER_DATA_DIR`), shared by the scenario files, which run in order on one
  worker. Set `JELLYRA_E2E_PROFILE` to an absolute path to reuse a profile while working on one
  scenario (`npx playwright test e2e/04-music.spec.ts`).
- Screenshots go to `e2e/screenshots/` (git-ignored), `<screen>-<lang>-light.png` and `-dark.png`.
  Light and dark are taken with the theme on "System" and the colour scheme emulated. A page capture
  cannot see the native Mica material, so the window surface colour is painted behind transparent
  areas.
- `npm run typecheck:e2e` type-checks the suite.
- The test server needs at least one movie with several audio tracks and text subtitles, one
  series, albums with four or more tracks, and ideally one song with lyrics.

### What every scenario checks besides its own steps

Renderer console errors and warnings (React warnings included), uncaught exceptions, main-process
stderr, failed requests and HTTP 4xx/5xx responses are all collected. A scenario fails on any
of them unless it is explicitly expected (bad address, wrong password, see "Known issues").

## Scenarios

| File                 | What is tested                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `01-onboarding`      | Unreachable address, an address that answers but is not Jellyfin, a pasted `/web/#/…` address being cut back to the server root, wrong password (error shown, field cleared), then sign-in and naming the server. The onboarding title stays readable in light and dark mode.                                                                                                                                                                                                |
| `02-servers`         | `accounts.json` holds an encrypted token (not the 32-hex Jellyfin token, no password) and the session survives a restart. Adding the same server again under another name; signing in again and closing the sheet at the naming step (the app must keep working); switcher menu; rename from Settings › Servers; Sign Out and Remove (back to onboarding); sign in again.                                                                                                    |
| `03-video`           | Watch Now, Movies grid, a movie detail, TV Shows grid, a series with seasons and episodes. Playing a movie: `currentTime` advances, +10 s, seek to 10 min, switch the audio track (playback resumes near the same position), turn a text subtitle on (a `<track>` shows), cast picker in the player, leave: the server's `UserData.PlaybackPositionTicks` matches, the detail page offers "Resume" and resuming starts there. The test movie's progress is reset afterwards. |
| `04-music`           | Listen Now, Albums, Artists, Songs, an album page, Play: the position advances. Next / Previous, Shuffle on/off, Repeat off → all → one → off, Now Playing sheet, Up Next queue, lyrics view on a track without lyrics (empty state, no 404 request), pause and close.                                                                                                                                                                                                       |
| `05-search`          | Ctrl+F focuses search; a movie, an artist and a song are found; the song plays from the results and its lyrics show (synced or plain); the "no results" state.                                                                                                                                                                                                                                                                                                               |
| `06-settings`        | Appearance Dark / Light / System (the explicit choice wins over the OS, System follows it), accent colour, video quality, audio and subtitle languages, autoplay, reset, music quality, About. French: applied immediately, kept after a restart with an English OS, and the main screens captured in French.                                                                                                                                                                |
| `07-cast-media-keys` | The Chromium features behind media keys and the Windows media flyout (SMTC) are enabled. `navigator.mediaSession` carries title, artist, album, artwork and playing/paused state. The handlers Chromium calls for media keys and flyout buttons (next, pause, play, previous) drive the player. The cast picker lists "This computer" as selected and stays inside the window once devices are discovered.                                                                   |

Unit tests were added where it made sense: `src/main/startup.test.ts`, `src/main/accounts/store.test.ts`
(async storage, fallback on removal, name kept on re-sign-in), `features/servers/servers.test.tsx`
(token switch on re-sign-in), `design/overlays/overlays.test.tsx` (Escape kept from the page).

## Bugs found and fixed

1. **Electron 45 deprecation errors on every launch and save.** `safeStorage.isEncryptionAvailable`,
   `encryptString` and `decryptString` are deprecated and log to stderr on each call. The account
   store now uses `isAsyncEncryptionAvailable` / `encryptStringAsync` / `decryptStringAsync` (with key
   rotation via `shouldReEncrypt`). `AccountStore.open()` is async and the IPC handlers are
   registered before the window loads.
2. **Two TanStack Query clients.** `App` created its own client while the session store cleared and
   configured another one. Switching or removing a server did not clear what the UI showed, and the
   4xx no-retry policy did not apply. The app now uses the session's client.
3. **Signing in again to the active server left the app on a revoked token.** Jellyfin revokes a
   device's previous token when it signs in again. If the "Add Server" sheet was closed at the naming
   step, the session kept the dead token and the next request ended in "Sign in required". The new
   token is now applied as soon as the sign-in succeeds.
4. **Signing in again to a saved server renamed it.** The custom name was replaced by the server's
   own name, even when the flow was abandoned. An empty name now keeps the existing one, and the
   naming step is pre-filled with it.
5. **Removing the active server sent the user to onboarding even with other servers saved.** The
   most recently used remaining server now becomes active.
6. **Onboarding unreadable in light mode.** The shell's immersive mode (used to hide the sidebar)
   painted the content area black, so the black title and labels were invisible. The video player
   already paints its own black stage, so the shell rule was removed.
7. **Escape in the video player's Audio / Subtitles / Cast menus left the movie.** The player's
   Escape shortcut ran before the popover's. Popovers now handle Escape in the capture phase and stop
   it there.
8. **Cast picker cut off at the bottom of the window.** It was placed once while it only listed
   "This computer"; discovered devices then grew it downwards off-screen. It is placed again
   whenever its size changes (ResizeObserver).
9. **A 404 for every track without lyrics.** Jellyfin 10.11 fills `HasLyrics` in item lists; the
   lyrics request is skipped when it is `false`, and the empty state shows directly.
10. **Track numbers misaligned with titles** in album and playlist lists (`height: 100%` disabled the
    row stretch and pinned the number to the top).
11. **Episodes row indented 40 px more than the season picker** (double page gutter).
12. **Search results indented 40 px more than the page title** (same double gutter).
13. **Now Playing (Lyrics / Up Next views): artwork narrower than the title, scrubber and volume
    below it.** The column now has the artwork's width.
14. **Settings › Playback: the streaming-quality hint was the list footer**, so it read as if it
    described the autoplay row. It is now the subtitle of its own row.
15. **Onboarding address placeholder clipped** ("…192.168.1.10" cut before the port). The port is
    dropped from the example: the hint below already says it is detected.
16. **Media keys / SMTC.** The `HardwareMediaKeyHandling` and `MediaSessionService` Chromium features
    are now enabled explicitly at startup (`src/main/startup.ts`, merged with any existing
    `--enable-features`). They are on by default in current Electron; this keeps them on if that
    changes.

### Jellyfin 10.11 compatibility of the data layer

Checked against 10.11.10, everything the app uses answers as expected: `/System/Info/Public`,
`/Users/AuthenticateByName`, `/Sessions/Logout`, `/UserViews`, `/Items` (with `userId`), `/Items/{id}`,
`/Items/Latest`, `/UserItems/Resume`, `/Shows/NextUp`, `/Shows/{id}/Seasons` and `/Episodes`,
`/Artists`, `/Items/{id}/InstantMix`, `/Audio/{id}/Lyrics` (404 when there are none, handled),
`/Audio/{id}/universal`, `/Items/{id}/PlaybackInfo`, HLS fMP4 transcoding, `/Videos/.../Subtitles/.../Stream.vtt`,
`/Sessions/Playing` (start, progress, stopped), `/UserPlayedItems` and `/UserFavoriteItems`.
No 10.9-only route is used.

## Known issues and what remains

- **Jellyfin 10.11 may answer 500 on the first HLS init segment (`/hls1/main/-1.mp4`) when a
  transcode starts mid-file** (audio track switch, resume). hls.js asks for the init segment and the
  first media segment in parallel, and the server races on starting the transcode. hls.js retries a
  second later and playback continues, so the video scenario tolerates exactly this response.
  Serialising the two requests in the app would mean patching hls.js's loader. Awaiting the
  "stopped" report before the new stream was tried: it did not remove the 500 and made track
  switches 2–4 s slower, so it was reverted.
- **One entry per server and user.** Adding the same server with the same user updates the existing
  entry instead of creating a second one. Two entries could not both work: Jellyfin revokes this
  device's previous token on each sign-in. "Switch" between two entries therefore needs a second
  account or server. It is covered by the store's unit tests, and the switcher menu (checked item)
  is exercised end to end.
- **Physical media keys and the Windows flyout cannot be pressed by Playwright.** The suite proves
  the Chromium features are on, the metadata and state are published, and the handlers Chromium
  calls drive the player. A last manual check on Windows: play a song, press the keyboard's
  play/pause and next keys, and open the volume flyout (Win+A / media overlay) to see title,
  artist and artwork.
- **Artwork loads slowly the first time** on this server: Jellyfin resizes images on demand. The
  neutral placeholders show meanwhile; it is not an app issue.
- **Minor accessibility points, left as is:** detail pages expose two `h1` elements (the visually
  hidden page title and the hero title); the mini player is a "Now Playing" region inside the
  shell's "Now Playing" region; single-choice settings menus use `menuitemcheckbox` rather than
  `menuitemradio`. None of them is visible.
- **The French onboarding placeholder** ("jellyfin.exemple.com ou 192.168.1.10") was not captured:
  onboarding only runs in English in the suite. It is the same length as the English one, which now
  fits.
- Live TV is out of scope and was not tested.
