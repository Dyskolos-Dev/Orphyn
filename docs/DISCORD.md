# Discord Rich Presence

Lumen shows what you play in your Discord status: music as **Listening to Lumen**, movies and
episodes as **Watching Lumen**, with the title, a progress bar and artwork.

## How it works

- Main process: `src/main/discord/`. It talks to the local Discord client over IPC through
  `@xhayper/discord-rpc`. The connection is lazy (first playback, or when Settings > Discord is
  opened) and retried every 30 s while Discord is not running, without any log noise.
- Renderer: `src/renderer/src/features/discord/` subscribes to the music player store and to the
  active video controller and sends a normalized presence over `window.lumen.discord`.
- Updates are sent on track change, play/pause and seeks, at most one per 2 s (Discord allows 5 per
  20 s). Paused playback drops the timestamps and shows "Paused" with the pause icon. Nothing
  playing, or turning the setting off, clears the activity at once.
- Settings > Discord: "Show what I am playing on Discord" (on by default), "Show artwork" (on by
  default) and a status line (Connected / Discord not running / Not configured).

## Artwork and privacy

The large image is the item artwork URL on your Jellyfin server
(`<server>/Items/<id>/Images/Primary?maxWidth=512`, album art for tracks, series poster for
episodes). Discord's servers fetch it, so it is only sent when "Show artwork" is on **and** the
server URL is `https` on a public host (not `localhost`, a LAN address or `.local`). Otherwise the
Lumen logo (asset `lumen`) is used. The link points to your server; it never contains a token or
`api_key` (such parameters are stripped before anything is sent).

## Application setup (owner)

1. Create an application named **Lumen** at <https://discord.com/developers/applications>. The
   name is what Discord displays after "Listening to" / "Watching".
2. Copy its **Application ID** into `DISCORD_CLIENT_ID` in `src/main/discord/config.ts`. It is a
   public identifier, not a secret. An empty value or `000000000000000000` keeps the feature inactive
   ("Not configured" in Settings).
3. Under **Rich Presence > Art Assets**, upload:
   - `lumen`: the logo, as a PNG of at least 1024 x 1024 px. Export it from
     `build/lumen-logo-source.png` (2048 px) resized to 1024 px, or use `build/icon.png`, which is
     already 1024 px.
   - `pause`: a small pause glyph (white, on a transparent or dark background), 512 px.
4. Asset keys must be exactly `lumen` and `pause` (see `ASSET_LOGO` / `ASSET_PAUSE` in `config.ts`).
   New assets can take a few minutes to appear in Discord.

## Testing it

Run Discord on the same machine, start Lumen with `npm run dev`, play a track, and check your
profile. In Discord, **Settings > Activity Privacy** must allow sharing the current activity.
