# Watch and listen together (SyncPlay)

Friends watch a film or listen to music in step, each on their own computer, through Jellyfin's
built-in SyncPlay (server 10.9+, checked on 10.11). A room ("group") lives on the server, so
everyone needs an account on **the same Jellyfin server**, with SyncPlay allowed for their user
(Dashboard > Users > SyncPlay access). Members can use Orphyn or the official Jellyfin web app in
the same room.

## Where it shows

- **Video player**: "Watch together" button (two-person icon) in the bottom-right controls; while
  in a room, a "Together · N" badge in the top bar, the speed menu and Picture in Picture are hidden
  (a room plays at normal speed, in the main window), and intros are not skipped automatically
  (members would fight over it; the Skip pill still works for everyone).
- **Mini player and Now Playing**: "Listen together" button; a count bubble shows the members; the
  badge sits in the Now Playing header.
- **Sheet** (one instance, `SyncPlayHost`): create a room (name prefilled "<user>'s room"), or join
  one from the server's list (refreshed every 5 s); in a room: members with their state, what plays,
  Leave.
- **Toasts**: "Sam joined", "Alex paused", "Waiting for Sam…", "Connection lost. Reconnecting…".

## Layers

| Folder / file                   | What it does                                                                                  |
| ------------------------------- | --------------------------------------------------------------------------------------------- |
| `jellyfin/syncplay/socket.ts`   | Server WebSocket `/socket?api_key=…&deviceId=…`: keep-alive, dead-link watchdog, reconnection |
| `jellyfin/syncplay/protocol.ts` | REST requests and the parsing of `SyncPlayCommand` / `SyncPlayGroupUpdate`                    |
| `jellyfin/syncplay/timeSync.ts` | Server clock offset from `/GetUtcTime` round trips                                            |
| `features/syncplay/logic.ts`    | Pure decisions: command scheduling, drift correction, members                                 |
| `features/syncplay/manager.ts`  | The room: membership, queue, commands, Buffering / Ready, rejoin (`syncPlayManager`)          |
| `features/syncplay/video.ts`    | `useSyncPlayVideo(controller, itemId, trailer)` for the video player                          |
| `features/syncplay/music.ts`    | The bridge to `musicPlayer` and its store                                                     |
| `features/syncplay/ui.tsx`      | Button, badge, sheet, host (registered as a shell `background` extension by `index.ts`)       |

The players are not changed. The video player uses the controller returned by `useSyncPlayVideo`
for every play / pause / seek of the person (buttons, keyboard, OS media keys); the controller's
`<video>` element is used only for the speed nudges. The music side watches the music store: a
change the person makes (play, pause, seek, next, previous, a new queue, "Add to queue", "Play
next") becomes a group request; what the room commands is applied through `musicPlayer`, flagged
so it is not taken for the person.

## Protocol (as seen on Jellyfin 10.11)

- Requests: `GET /SyncPlay/List`, `POST /SyncPlay/New {GroupName}`, `/Join {GroupId}`, `/Leave`,
  `/SetNewQueue {PlayingQueue, PlayingItemPosition, StartPositionTicks}`, `/Queue {ItemIds, Mode}`,
  `/SetPlaylistItem {PlaylistItemId}`, `/NextItem` and `/PreviousItem {PlaylistItemId}` (ignored
  unless the id is still the playing entry, so every member may send it at the end of an item),
  `/Unpause`, `/Pause`, `/Stop`, `/Seek {PositionTicks}`, `/Buffering` and `/Ready {When,
PositionTicks, IsPlaying, PlaylistItemId}`, `/Ping {Ping}`, `/SetIgnoreWait {IgnoreWait}`.
- Socket messages: `ForceKeepAlive` (Data = timeout in s; we send `KeepAlive` every half of it),
  `SyncPlayCommand` (`Unpause | Pause | Seek | Stop`, `When`, `PositionTicks`, `PlaylistItemId`) and
  `SyncPlayGroupUpdate` (`GroupJoined`, `GroupLeft`, `UserJoined`, `UserLeft`, `StateUpdate`,
  `PlayQueue`, and the errors `NotInGroup`, `GroupDoesNotExist`, `CreateGroupDenied`,
  `JoinGroupDenied`, `LibraryAccessDenied`). Dates have 7 fractional digits; guids come with and
  without dashes (we compare them normalised).
- The socket must be open before `/New` or `/Join`: the answer comes as `GroupJoined` on it.
- A new room sends `Stop` (empty playlist id) right after `GroupJoined`; we ignore it when we are
  about to share what we play.
- After `SetNewQueue`, every member loads the item and reports `Ready`; when all are ready the
  server sends `Unpause` with a `When` about a second ahead. A `Seek` puts the room in `Waiting`
  until everyone is ready again. `Unpause` is re-sent to whoever reports `Ready` again: that is how a
  late member resyncs.
- **When a session's socket closes, the server takes it out of its room** (and deletes the room if
  it was alone).

## Time and sync

- Clock: a burst of 8 `/GetUtcTime` samples (300 ms apart), then one every 30 s; 16 kept. Each
  gives offset and round trip (NTP formula); the estimate is the median offset of the half with the
  lowest round trip. The round trip goes to the server with `/Ping`.
- Commands are applied at `When` converted to local time. A late `Unpause` starts now, further in
  by the lateness. `Pause` stops at `When`, then sits on `PositionTicks` (everyone on the same
  frame).
- Drift (every 500 ms while playing, after a 1.5 s settling time): expected position = last
  `Unpause` position + elapsed server time. Under 40 ms: normal speed. Up to 1 s: speed nudged
  between 0.95 and 1.05 (absorbs it in about 2 s, no glitch). Beyond: a seek. The music output has
  no speed control in its public API, so music only seeks, beyond 0.5 s.
- Buffering / Ready: after a new queue, a seek or (re)joining, the player is held paused on the
  room's position and `Ready` is sent once it can play there. A stall longer than 0.5 s while the
  room plays sends `Buffering`, and `Ready` when it recovers, so the room waits for the slowest.

## Members

The server gives the members' **names** only (no ids, no per-member state, no author of a
command). So: our own state is exact; when the room waits while we are ready and there is one other
member, it is them ("Waiting for Sam…"); with more people it says "the others". Likewise "Alex
paused" names the author only when there is exactly one other member. Avatars: ours from the
server, initials on a system colour for the others (`/Users/Public` is empty on most servers).

## Edge cases

- **Network loss**: the socket reconnects (1 s, 2 s, 4 s… up to 30 s, ±20 %; at once when the OS
  says it is back online; a link silent for 2.5 keep-alive periods counts as dead). Playback goes on
  meanwhile; once back, we join the room again and it resyncs us. If we were alone, the room is gone:
  it is opened again with the same name and what we play.
- **Room closed** (`GroupLeft` we did not ask for, `GroupDoesNotExist`): toast, out of the room,
  playback goes on alone.
- **Item not available** to this user (another library, parental rating): it is not in the answer
  of `/Items?ids=`; toast, and `SetIgnoreWait true` so the room does not wait for us; the next item
  we can play clears it. `LibraryAccessDenied` (someone cannot play what we shared): toast.
- **Leaving mid-playback**: `/Leave`, speed back to 1, playback continues here on its own.
- **Server switch / sign-out**: we leave the room.
- Closing the video player keeps you in the room; if the room starts another item, the player
  opens again on it.
- A video ending with a next episode: the room moves on together (the first member of the list
  asks, the others follow).

## Tests

- `jellyfin/syncplay/*.test.ts`: message parsing (recorded 10.11 messages), REST bodies, clock
  estimator, socket keep-alive / backoff / reconnection.
- `features/syncplay/logic.test.ts`, `music.test.ts`: scheduling, drift decisions, members, queue
  diffs, the music intents.
- `features/syncplay/syncplay.live.test.ts` (opt-in): two members (two module instances of the
  manager, two devices) with simulated players against a real server: create, join, synced start,
  pause on the same frame, seek, drift corrected by speed, network loss and rejoin, leave.

```
ORPHYN_LIVE_URL=https://server ORPHYN_LIVE_USER=user ORPHYN_LIVE_PASSWORD=… \
  npx vitest run src/renderer/src/features/syncplay/syncplay.live.test.ts
```
