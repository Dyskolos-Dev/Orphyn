# Casting backend

Main-process casting for Orphyn: Google Cast, UPnP/DLNA (including WiiM / LinkPlay) and AirPlay
(best effort). The renderer only sees `window.orphyn.cast` (typed in `src/shared/ipc/cast.ts`).
UI and player integration are a separate task.

## Architecture

```
renderer ── window.orphyn.cast ── preload/cast.ts ── ipcMain (main/cast/index.ts)
                                                          │
                                                     CastManager ── Discovery ── DeviceRegistry
                                                          │            (mDNS + SSDP, expiry)
                                                     CastSession (one per device)
                                                          │  state machine, command queue,
                                                          │  reconnection, "ended" detection
                                                     CastTransport
                                  ┌───────────────────────┼───────────────────────┐
                           ChromecastTransport      UpnpTransport           AirplayTransport
                           (castv2-client DMR)      (SOAP AVTransport/      (legacy HTTP /play)
                                                     RenderingControl)
```

| File                       | Role                                                                    |
| -------------------------- | ----------------------------------------------------------------------- |
| `src/shared/cast/model.ts` | `CastDevice`, `CastMedia`, `CastSessionState`, `CastCommand`            |
| `src/shared/ipc/cast.ts`   | channel names, `CastApi`, event payloads                                |
| `main/cast/discovery.ts`   | mDNS `_googlecast`/`_airplay`/`_raop`, SSDP MediaRenderer + description |
| `main/cast/registry.ts`    | live device set, 90 s expiry (swept every 10 s), explicit removal       |
| `main/cast/session.ts`     | state machine; never emits `ended` after a user stop; reconnects        |
| `main/cast/transports/*`   | one protocol each, behind the `CastTransport` interface                 |
| `main/cast/didl.ts` etc.   | pure builders/parsers (DIDL-Lite, SOAP, descriptions, TXT, XML helper)  |

No XML dependency was added: `xml.ts` has a small escaper and a tolerant parser.

### Discovery lifecycle

A short scan (8 s) runs at app start so the list is warm. After that, discovery runs only between
`startDiscovery()` and `stopDiscovery()`. While running it re-queries every 20 s; devices not seen
for 90 s (or announced as gone via mDNS goodbye) disappear and `onDevicesChanged` fires with the full
list. Sessions survive discovery stopping.

### Sessions and robustness

- Commands per device are serialized; every network call has a timeout (connect 10 s, command 10 s,
  SOAP 5 s) and rejects instead of hanging.
- If the link drops during playback the session goes to `connecting`, then reloads the media at the
  last known position (attempts after 0.5 s, 2 s, 5 s). After that: status `error` + `onError`.
- The end of a track emits `onEnded` exactly once: Cast `idleReason FINISHED`; UPnP `STOPPED` after
  having seen `PLAYING`; AirPlay playback-info going empty after playing. A user `stop()` never
  produces `ended`.
- App quit (`before-quit`) disposes discovery, polling timers and sockets. All logs are prefixed
  `[cast]`. No handler lets a rejection escape.

## Supported devices and limits

| Device                                 | Protocol                       | Audio  | Video  | Volume / mute              |
| -------------------------------------- | ------------------------------ | ------ | ------ | -------------------------- |
| Chromecast, Google Home/Nest, Cast TVs | Cast v2 Default Media Receiver | yes    | yes¹   | yes                        |
| WiiM / LinkPlay streamers              | UPnP AVTransport (flag `wiim`) | yes    | no     | yes                        |
| Other DLNA/UPnP MediaRenderers         | UPnP                           | yes    | maybe² | if RenderingControl exists |
| Apple TV and AirPlay-video receivers   | legacy AirPlay HTTP            | yes³   | yes³   | **no**                     |
| HomePod, AirPlay 2 speakers, Macs      | AirPlay 2 / RAOP               | **no** | **no** | –                          |

1. Video capability comes from the `ca` TXT bit field, else guessed from the model (speakers/groups
   are audio-only).
2. UPnP video is only advertised when the device type/model suggests a TV or display.
3. Best effort: `POST /play` with `Content-Location`, polling `/playback-info`, `/rate`, `/scrub`,
   `/stop`. Works on Apple TVs without a pairing PIN/password. Receivers that require pairing (tvOS
   with "Require device verification") fail at `load` with an error.

### AirPlay: what is realistic from Node

- **AirPlay 2 audio** (HomePod, AirPlay 2 speakers) needs HomeKit pairing (SRP + Ed25519/Curve25519)
  and an encrypted RTSP/RAOP stream with PTP clock sync. Not feasible without a dedicated library,
  and none is installed. Such devices are listed with `capabilities: {audio:false, video:false}` so a
  UI can show them as unsupported; `load` rejects with a clear message.
- **RAOP-only (`_raop._tcp`)** devices are listed as unsupported for the same reason.
- **macOS receivers** advertise the video feature bit but do not implement the legacy HTTP API, so
  they are marked unsupported.
- Legacy remote volume does not exist in this protocol: `setVolume`/`setMuted` reject.
- If a library such as `airtunes2`/`node-airplay2` is approved later, it can be added behind the
  same `CastTransport` interface without touching the rest.

### UPnP / WiiM notes

- `SetAVTransportURI` carries DIDL-Lite with title, artist, album, `albumArtURI`, duration and a
  `protocolInfo` (`http-get:*:<mime>:DLNA.ORG_PN=…;DLNA.ORG_OP=01;DLNA.ORG_FLAGS=…`).
- Position is polled every second (`GetTransportInfo` + `GetPositionInfo`), volume every ~5 s.
  Five consecutive poll failures count as a lost link (reconnect logic above).
- WiiM devices are detected from manufacturer `Linkplay`/`WiiM` (or model containing `WiiM`) and
  are controlled via UPnP; they may also appear as Cast devices when Cast is enabled on them.
- The device must be able to reach the stream URL (the Jellyfin server address and `api_key` in the
  URL must be valid from the device's network; `localhost` will not work).

## Renderer usage

```ts
const cast = window.orphyn.cast

const offDevices = cast.onDevicesChanged((devices) => setDevices(devices))
const offSession = cast.onSessionChanged((s) => setSession(s))
const offEnded = cast.onEnded(({ deviceId }) => playNextInQueueOn(deviceId))
const offError = cast.onError(({ message }) => toast(message))

await cast.startDiscovery() // when the cast picker opens
setDevices(await cast.listDevices())

await cast.load(deviceId, {
  url: streamUrlForCast(item), // absolute, with api_key
  mimeType: 'audio/flac',
  kind: 'audio', // 'video' only for Cast devices with capabilities.video
  title,
  artist,
  album,
  artworkUrl,
  durationSec,
  startPositionSec: 0,
})
await cast.pause(deviceId)
await cast.seek(deviceId, 90)
await cast.setVolume(deviceId, 0.4) // 0..1
await cast.stop(deviceId)
await cast.disconnect(deviceId) // stops playback and releases the connection
await cast.stopDiscovery() // when the picker closes
```

Session state (`CastSessionState`): `status` is `idle | connecting | playing | paused | buffering |
error`; `positionSec`/`durationSec` update about once per second while playing; `volume` is 0..1.
Commands reject with an `Error` (message is safe to show) when the device is unreachable or refuses.

IPC channels (`CAST_CHANNELS`): `cast:start-discovery`, `cast:stop-discovery`, `cast:list-devices`,
`cast:get-session`, `cast:load`, `cast:play`, `cast:pause`, `cast:seek`, `cast:stop`,
`cast:set-volume`, `cast:set-muted`, `cast:disconnect`; events `cast:devices-changed`,
`cast:session-changed`, `cast:ended`, `cast:error`.

## Manual check (no Electron needed)

`src/main/cast/cli.ts` bundles with the already-installed esbuild:

```sh
npx esbuild src/main/cast/cli.ts --bundle --platform=node --format=cjs --packages=external \
  --alias:@shared=./src/shared --outfile=out/cast-cli.cjs

node out/cast-cli.cjs list 6                       # list devices found in 6 s
node out/cast-cli.cjs play "WiiM" http://host/song.mp3 audio/mpeg 20   # play 20 s, print state, stop
```

`play` matches the device by name or id fragment, sends a test URL with metadata, prints every
state change (`state: playing 3/180s vol=0.4`), then stops and exits. Use a URL the device can
reach (LAN address, not `localhost`).

## Tests

`npx vitest run src/main/cast` covers DIDL-Lite, SOAP, device descriptions, Cast/AirPlay TXT,
UPnP polling and ended logic (mocked SOAP), the session state machine and reconnection (mocked
transports, fake timers), the registry expiry and the manager's discovery policy. Real-device
behaviour (Cast/UPnP/AirPlay on hardware) is not covered by automated tests: use the manual check.
