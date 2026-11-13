# Picture in Picture and OS media controls (Tauri shell)

Everything here is Tauri-only: Electron has neither the PiP windows nor the native media controls
(it keeps Chromium's `navigator.mediaSession`).

## Music mini player (PiP)

A small always-on-top, frameless, rounded window (320 x 380, resizable down to 240 x 285, the
aspect ratio is kept) showing the artwork full-bleed over a blurred copy of it, the title and
artist, a thin progress bar (click to seek) and, on hover, favourite, previous, play / pause, next,
"Back to Orphyn" and close. Drag it anywhere; its position and size are remembered
(`pip.json` in the app data folder) and reset to the bottom-right corner if that screen is gone.

- Open it with the PiP button in the mini player, in Now Playing, or **Ctrl/Cmd + Shift + M**
  (the same shortcut closes it).
- The player stays in the main window. The PiP window is the same renderer loaded with
  `index.html?pip=music`; `bridge/pip.ts` mounts `features/pip/entry.tsx` there instead of the app,
  so a PiP window has no audio engine, Discord sync or media session of its own.
- Sync is plain Tauri events, no Rust relay (`features/pip/protocol.ts`):
  `orphyn:pip:music-state` (main -> PiP: a snapshot, resent when the track, play state, favourite
  or duration change, after a seek, and every 2 s while playing; the PiP moves the bar on its own in
  between), `orphyn:pip:music-command` (PiP -> main: toggle, next, previous, seek, favourite) and
  `orphyn:pip:ready` (a new window asks for the current snapshot). `useMusicPipHost` (mounted by the
  mini player) is the main-window side. Minimising the main window does not stop it.
- Light / dark and the interface style (`html[data-theme]`, `html[data-style]`) come from the same
  `localStorage` as the main window; the PiP re-applies them on the `storage` event. macOS adds a
  HUD vibrancy behind the window; elsewhere the rounded card and the OS shadow carry the look.

## Video PiP

The PiP button of the video controls and **Ctrl/Cmd + Shift + P**:

1. Where the webview has Picture in Picture (WebView2, WKWebView) the `<video>` element goes
   native (`requestPictureInPicture`, or `webkitSetPresentationMode` on macOS). It is the same
   element, so the player keeps reporting progress to Jellyfin and nothing is duplicated.
2. Otherwise (WebKitGTK on Linux) a custom always-on-top video window opens
   (`index.html?pip=video`, 480 x 270, 16:9 kept, play / pause, seek bar, back, close on hover).
   Hand-over (`features/pip/videoHandover.ts`, `features/video/player/useVideoPip.ts`): the main
   player pauses and sends `orphyn:pip:video-start` (item, position, audio / subtitle track); the PiP
   window plays it with its own `VideoPlaybackController`, reports `orphyn:pip:video-progress` every
   second, and when it closes the main player seeks to the last reported position and resumes only if
   the PiP was playing. Reporting to Jellyfin is sequential, never two playing sessions: the main
   player reports "paused" when it hands over, the PiP reports its own session, then stops it on
   close before the main player reports playing again.
   Text subtitles are not drawn in the custom window; a burned-in subtitle track is.

Only one PiP window exists at a time (opening the video one closes the music one, and leaving the
video page closes its PiP).

## Native OS media controls

`src-tauri/src/media_controls/` (`souvlaki`): Windows SMTC, macOS Now Playing, Linux MPRIS. The
renderer publishes metadata (title, artist, album, artwork URL, duration) and playback (status,
position) through `bridge/mediaControls.ts`; the OS buttons come back as
`orphyn:media-controls:action` (play, pause, toggle, next, previous, stop, seek, seekBy).

The OS has one media session per app, and there are two players. `claimMediaControls` is the hub:
the video player claims the session while it is open and releases it on leave, which hands it back
to the music player (which republishes). Whoever holds it receives the buttons. Where the shell has
native controls neither player touches `navigator.mediaSession`, so the OS panel never shows two
entries; Electron and the browser keep `navigator.mediaSession`.

## Windows

- The PiP windows get their own capability, added at run time (`pip::init`): events, dragging,
  closing and bringing the main window back, nothing else.
- Windows debug builds: WebView2 needs the same browser arguments for every window of the profile,
  so the PiP windows repeat the `ORPHYN_CDP_PORT` argument the main window uses.
- Window creation runs in an `async` command: creating a window from a synchronous command
  deadlocks on Windows.

## Tests

`cargo test pip media_controls discord`; `npx vitest run src/renderer/src/features/pip` (state sync
reducer, position extrapolation, video hand-over and return position).
