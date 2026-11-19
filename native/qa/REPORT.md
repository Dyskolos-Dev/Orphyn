# Native QA report (lean pass)

Status per screen. "not compared" means no capture pair was made in this pass (quota-limited);
it is not a claim of parity.

| Screen | Result |
|---|---|
| Settings > Servers, "Actif" badge | fixed (override in `styles/overrides/settings.css`: inline-block + physical left margin, since Blitz drops padding / logical margin on an inline span in a text run). Built and clippy/test green; not yet confirmed by a live capture. |
| Music home, album, artist | not compared |
| Now Playing (player, lyrics, queue) | not compared |
| "..." menu | not compared |
| Video home, movie, series, player controls | not compared |
| Search, Ton annee | not compared |
| Settings Appearance / Music / About | not compared |
| Sidebar | not compared |

## Live checks requested, not done
- Switching to Minimal (root cause of the guarded panic)
- PiP music + video visible
- Full-screen Now Playing smoothness

These need an interactive Windows session with the test server; they remain open.
