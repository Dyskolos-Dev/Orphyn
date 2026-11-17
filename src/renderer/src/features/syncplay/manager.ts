/**
 * SyncPlay group membership and playback following (one instance, `syncPlayManager`).
 *
 * - Connects the server socket while in a group, keeps the server clock estimate fresh.
 * - Applies the group's queue (opens the video player or loads the music queue) and its commands
 *   at their `When` time; corrects drift while playing; reports Buffering / Ready so the group
 *   waits for its slowest member.
 * - Turns local play / pause / seek / next / new queue into group requests (see video.ts,
 *   music.ts), and survives a network loss by rejoining.
 *
 * UI state lives in the `useSyncPlay` store; one-off events (toasts) go through `onNotice`.
 */
import type { Api } from '@jellyfin/sdk'
import { create } from 'zustand'
import {
  syncPlay as sp,
  useSession,
  type BaseItemDto,
  type GroupState,
  type PlayQueue,
  type QueueEntry,
  type SocketStatus,
  type SyncPlayCommand,
  type SyncPlayErrorKind,
  type SyncPlayEvent,
  type SyncPlayGroup,
} from '@renderer/jellyfin'
import {
  TICKS_PER_SECOND,
  commandKey,
  describeMembers,
  driftAction,
  expectedPosition,
  likelyAuthor,
  planCommand,
  waitingFor,
  type Member,
} from './logic'
import type { Notice, SyncPlayer } from './types'

export type Phase = 'off' | 'connecting' | 'joined' | 'reconnecting'
export type MediaKind = 'video' | 'audio'

export interface SyncPlayState {
  phase: Phase
  group: SyncPlayGroup | null
  groupState: GroupState
  selfName: string
  selfAvatar: string | undefined
  selfReady: boolean
  members: Member[]
  queue: PlayQueue | null
  /** The group's current item, as this user sees it (null: none, or not available to us). */
  currentItem: BaseItemDto | null
  currentKind: MediaKind | null
  sheetOpen: boolean
  sheetKind: MediaKind
}

const initialState: SyncPlayState = {
  phase: 'off',
  group: null,
  groupState: 'Idle',
  selfName: '',
  selfAvatar: undefined,
  selfReady: false,
  members: [],
  queue: null,
  currentItem: null,
  currentKind: null,
  sheetOpen: false,
  sheetKind: 'video',
}

export const useSyncPlay = create<SyncPlayState>()(() => initialState)

/** Local intents of the music player, forwarded to the group (see music.ts). */
export interface MusicIntents {
  play(): void
  pause(): void
  seek(seconds: number): void
  next(currentItemId: string | null): void
  previous(currentItemId: string | null): void
  jump(itemId: string): void
  newQueue(itemIds: string[], index: number, startSeconds: number): void
}

/** What the manager needs from the music player (music.ts). */
export interface MusicBridge {
  readonly player: SyncPlayer
  /** Item ids of the local queue and the current index. */
  localQueue(): { ids: string[]; index: number; playing: boolean }
  /** Replaces the local queue (paused). */
  load(tracks: BaseItemDto[], index: number): void
  /** Moves to another entry of the same queue (paused). */
  jump(index: number): void
  /** The group's queue grew around the current entry: add the new tracks locally if they fit. */
  extend(tracks: BaseItemDto[]): void
  start(intents: MusicIntents): void
  stop(): void
}

type LocalKind = 'pause' | 'unpause' | 'seek' | 'queue'

/** A command reported by `Ready` is played this long after (ms) without any drift fighting. */
const GRACE_MS = 1500
const BUFFERING_DEBOUNCE_MS = 500
const MINE_WINDOW_MS = 5000
const WAITING_NOTICE_GAP_MS = 6000

const playingEntry = (q: PlayQueue | null): QueueEntry | null =>
  q ? (q.playlist[q.playingItemIndex] ?? null) : null

export class SyncPlayManager {
  private api: Api | null = null
  private userId = ''
  private socket: sp.JellyfinSocket | null = null
  private time: sp.TimeSync | null = null
  private offs: (() => void)[] = []
  private videoPlayer: SyncPlayer | null = null
  private music: MusicBridge | null = null
  private playerOffs = new Map<SyncPlayer, () => void>()
  private navigate: ((path: string) => void) | null = null
  private readonly noticeListeners = new Set<(n: Notice) => void>()

  private seen: string[] = []
  private commandTimer: ReturnType<typeof setTimeout> | null = null
  private bufferTimer: ReturnType<typeof setTimeout> | null = null
  private driftTimer: ReturnType<typeof setInterval> | null = null
  /** Playback may run (an Unpause applied); otherwise the player is held paused. */
  private allowPlay = false
  private lastUnpause: { when: number; positionTicks: number } | null = null
  /** A Ready is owed to the server (after a new queue, a seek, joining). */
  private awaitingReady = false
  private reportedReady: boolean | null = null
  private graceUntil = 0
  private rate = 1
  private rateTarget: SyncPlayer | null = null
  private lastLocal: { kind: LocalKind; at: number } | null = null
  private items = new Map<string, BaseItemDto>()
  private queueKey = ''
  private pendingCommand: SyncPlayCommand | null = null
  private shareOnJoin = false
  private leaving = false
  private rejoining = false
  private ignoringWait = false
  private lastWaitingNotice = 0
  private othersSeen = 0

  // ----------------------------------------------------------------------- wiring

  setMusicBridge(bridge: MusicBridge): void {
    this.music = bridge
  }

  /** The always-mounted host gives the router's navigate (to open the video player). */
  setNavigator(navigate: ((path: string) => void) | null): void {
    this.navigate = navigate
  }

  onNotice(listener: (n: Notice) => void): () => void {
    this.noticeListeners.add(listener)
    return () => this.noticeListeners.delete(listener)
  }

  private notify(n: Notice): void {
    for (const l of [...this.noticeListeners]) l(n)
  }

  get active(): boolean {
    const phase = useSyncPlay.getState().phase
    return phase === 'joined' || phase === 'reconnecting'
  }

  openSheet(kind: MediaKind): void {
    useSyncPlay.setState({ sheetOpen: true, sheetKind: kind })
  }

  closeSheet(): void {
    useSyncPlay.setState({ sheetOpen: false })
  }

  // ----------------------------------------------------------------------- groups

  async listGroups(): Promise<SyncPlayGroup[]> {
    const { api } = useSession.getState()
    if (!api) return []
    return sp.listGroups(api)
  }

  async create(name: string): Promise<void> {
    try {
      const api = await this.ensureConnected()
      this.shareOnJoin = true
      await sp.createGroup(api, name)
    } catch {
      this.failToConnect()
    }
  }

  async join(groupId: string): Promise<void> {
    try {
      const api = await this.ensureConnected()
      this.shareOnJoin = false
      await sp.joinGroup(api, groupId)
    } catch {
      this.failToConnect()
    }
  }

  /** Leaves the group; local playback goes on, on its own. */
  async leave(): Promise<void> {
    const api = this.api
    this.leaving = true
    try {
      if (api) await sp.leaveGroup(api)
    } catch {
      /* not in a group any more (403) or offline: leaving locally is enough */
    }
    this.teardown()
    this.notify({ kind: 'left' })
  }

  private async ensureConnected(): Promise<Api> {
    if (this.api && this.socket) {
      await this.socket.whenOpen()
      return this.api
    }
    const { api, account, deviceId } = useSession.getState()
    if (!api || !account || !deviceId || !api.accessToken) throw new Error('No session')
    this.api = api
    this.userId = account.userId
    useSyncPlay.setState({ phase: 'connecting', selfName: account.userName })
    void this.loadSelf(api, account.userId)

    const socket = new sp.JellyfinSocket(sp.socketUrl(api.basePath, api.accessToken, deviceId))
    this.socket = socket
    this.offs.push(
      socket.onMessage((m) => {
        const event = sp.parseSyncPlayMessage(m)
        if (event) this.onEvent(event)
      }),
      socket.onStatus((s) => this.onSocketStatus(s)),
      // Another server (or a sign-out): this group is over for us.
      useSession.subscribe((s) => {
        if (s.api !== this.api && this.api) void this.leave()
      }),
    )
    socket.connect()
    await socket.whenOpen()

    this.time = new sp.TimeSync({
      fetchUtc: () => sp.getUtcTime(api),
      onUpdate: (est) => {
        // The server places `When` far enough ahead for the slowest member's ping.
        if (est.count === 1 || est.count % 4 === 0) void sp.sendPing(api, est.delay).catch(() => {})
      },
    })
    this.time.start()
    this.driftTimer = setInterval(() => this.correctDrift(), 500)
    this.music?.start(this.musicIntents)
    return api
  }

  private async loadSelf(api: Api, userId: string): Promise<void> {
    try {
      const me = await sp.fetchSelf(api)
      useSyncPlay.setState({
        selfName: me.name || useSyncPlay.getState().selfName,
        selfAvatar: sp.userAvatarUrl(api, userId, me.imageTag),
      })
      this.refreshMembers()
    } catch {
      /* initials then */
    }
  }

  private failToConnect(): void {
    if (useSyncPlay.getState().phase === 'connecting') {
      this.teardown()
      this.notify({ kind: 'error' })
    }
  }

  private teardown(): void {
    this.offs.splice(0).forEach((off) => off())
    this.socket?.close()
    this.socket = null
    this.time?.stop()
    this.time = null
    if (this.driftTimer) clearInterval(this.driftTimer)
    this.driftTimer = null
    this.clearCommandTimer()
    if (this.bufferTimer) clearTimeout(this.bufferTimer)
    this.bufferTimer = null
    this.setRate(1)
    this.music?.stop()
    this.api = null
    this.allowPlay = false
    this.lastUnpause = null
    this.awaitingReady = false
    this.reportedReady = null
    this.queueKey = ''
    this.pendingCommand = null
    this.items.clear()
    this.seen = []
    this.shareOnJoin = this.leaving = this.rejoining = this.ignoringWait = false
    this.othersSeen = 0
    const { sheetOpen, sheetKind } = useSyncPlay.getState()
    useSyncPlay.setState({ ...initialState, sheetOpen, sheetKind })
  }

  // ----------------------------------------------------------------------- players

  /** The video player registers itself while mounted (and in a group). */
  attachVideo(player: SyncPlayer): () => void {
    this.videoPlayer = player
    this.watch(player)
    const entry = playingEntry(useSyncPlay.getState().queue)
    if (
      useSyncPlay.getState().currentKind === 'video' &&
      entry &&
      player.itemId() === entry.itemId
    ) {
      this.align(null)
    }
    return () => {
      if (this.rateTarget === player) this.setRate(1)
      this.playerOffs.get(player)?.()
      this.playerOffs.delete(player)
      if (this.videoPlayer === player) this.videoPlayer = null
    }
  }

  /** The player the group's current item plays in. */
  private get player(): SyncPlayer | null {
    const kind = useSyncPlay.getState().currentKind
    if (kind === 'video') return this.videoPlayer
    if (kind === 'audio') return this.music?.player ?? null
    return null
  }

  private watch(player: SyncPlayer): void {
    if (this.playerOffs.has(player)) return
    this.playerOffs.set(
      player,
      player.subscribe(() => {
        if (player === this.player) this.onPlayerChange()
      }),
    )
  }

  /** Pauses on the group's position and owes the server a Ready once loaded there. */
  private align(startSeconds: number | null): void {
    const p = this.player
    if (!p) return
    this.allowPlay = false
    this.lastUnpause = null
    this.setRate(1)
    p.pause()
    if (startSeconds !== null && Math.abs(p.position() - startSeconds) > 0.25) p.seek(startSeconds)
    this.awaitingReady = true
    this.reportedReady = null
    this.graceUntil = Date.now() + GRACE_MS
    this.onPlayerChange()
  }

  private onPlayerChange(): void {
    const p = this.player
    const entry = playingEntry(useSyncPlay.getState().queue)
    if (!p || !entry) return
    if (p.itemId() !== entry.itemId) {
      // Another item than the group's (being switched): it waits for the group.
      if (p.isPlaying()) p.pause()
      this.setSelfReady(false)
      return
    }
    const ready = p.isReady()
    this.setSelfReady(ready)
    if (!this.allowPlay && p.isPlaying()) p.pause()
    if (this.awaitingReady) {
      if (ready) {
        this.awaitingReady = false
        this.report(true)
      }
      return
    }
    if (ready) {
      if (this.bufferTimer) clearTimeout(this.bufferTimer)
      this.bufferTimer = null
      if (this.reportedReady === false) this.report(true)
      return
    }
    // Stalled while the group plays: tell it to wait for us (unless it is our own seek settling).
    if (this.reportedReady === false || this.bufferTimer) return
    if (useSyncPlay.getState().groupState !== 'Playing' || !this.allowPlay) return
    const wait = Math.max(BUFFERING_DEBOUNCE_MS, this.graceUntil - Date.now())
    this.bufferTimer = setTimeout(() => {
      this.bufferTimer = null
      const cur = this.player
      if (cur && !cur.isReady() && this.allowPlay) this.report(false)
    }, wait)
  }

  private setSelfReady(ready: boolean): void {
    if (useSyncPlay.getState().selfReady === ready) return
    useSyncPlay.setState({ selfReady: ready })
    this.refreshMembers()
  }

  private report(ready: boolean): void {
    const api = this.api
    const p = this.player
    const entry = playingEntry(useSyncPlay.getState().queue)
    if (!api || !p || !entry || !this.time) return
    this.reportedReady = ready
    const body = {
      when: this.time.serverNow(),
      positionTicks: p.position() * TICKS_PER_SECOND,
      isPlaying: p.isPlaying(),
      playlistItemId: entry.playlistItemId,
    }
    void (ready ? sp.reportReady(api, body) : sp.reportBuffering(api, body)).catch(() => {})
  }

  private setRate(rate: number): void {
    const p = this.player
    if (this.rateTarget && this.rateTarget !== p) this.rateTarget.setRate?.(1)
    this.rateTarget = p
    this.rate = rate
    p?.setRate?.(rate)
  }

  private correctDrift(): void {
    const p = this.player
    const last = this.lastUnpause
    const entry = playingEntry(useSyncPlay.getState().queue)
    if (!p || !last || !entry || !this.allowPlay || !this.time?.ready) {
      if (this.rate !== 1) this.setRate(1)
      return
    }
    if (p.itemId() !== entry.itemId || !p.isPlaying() || !p.isReady()) return
    if (Date.now() < this.graceUntil) return
    const expected = expectedPosition(last, this.time.serverNow())
    const action = driftAction(p.position() - expected, Boolean(p.setRate))
    if (action.kind === 'rate') {
      if (action.rate !== this.rate) this.setRate(action.rate)
    } else if (action.kind === 'seek') {
      this.setRate(1)
      p.seek(expected)
      this.graceUntil = Date.now() + GRACE_MS
    }
  }

  // ----------------------------------------------------------------------- local intents

  private mark(kind: LocalKind): void {
    this.lastLocal = { kind, at: Date.now() }
  }

  private isMine(kind: LocalKind): boolean {
    return (
      this.lastLocal !== null &&
      this.lastLocal.kind === kind &&
      Date.now() - this.lastLocal.at < MINE_WINDOW_MS
    )
  }

  private send(run: (api: Api) => Promise<void>): void {
    const api = this.api
    if (!api) return
    void run(api).catch(() => this.notify({ kind: 'error' }))
  }

  requestPlay(): void {
    if (!playingEntry(useSyncPlay.getState().queue)) {
      this.shareLocal(true)
      return
    }
    this.mark('unpause')
    this.send(sp.requestUnpause)
  }

  requestPause(): void {
    this.mark('pause')
    // Stop right away; the Pause command then puts everyone on the same frame.
    this.allowPlay = false
    this.player?.pause()
    this.send(sp.requestPause)
  }

  requestSeek(seconds: number): void {
    this.mark('seek')
    this.send((api) => sp.requestSeek(api, seconds * TICKS_PER_SECOND))
  }

  requestNext(): void {
    const entry = playingEntry(useSyncPlay.getState().queue)
    if (!entry) return
    this.mark('queue')
    this.send((api) => sp.nextItem(api, entry.playlistItemId))
  }

  requestPrevious(): void {
    const entry = playingEntry(useSyncPlay.getState().queue)
    if (!entry) return
    this.mark('queue')
    this.send((api) => sp.previousItem(api, entry.playlistItemId))
  }

  /** Adds to the group's queue without touching what plays. */
  queueItems(itemIds: readonly string[], mode: 'Queue' | 'QueueNext'): void {
    if (itemIds.length === 0) return
    this.mark('queue')
    this.send((api) => sp.queueItems(api, itemIds, mode))
  }

  /** The group's video ended here, with `nextId` (next episode) to follow: one request for all. */
  advanceAtEnd(nextId: string): void {
    const s = useSyncPlay.getState()
    const q = s.queue
    if (q && q.playingItemIndex >= 0 && q.playingItemIndex < q.playlist.length - 1) {
      this.requestNext() // deduplicated by the server
      return
    }
    // Everyone gets here at once: the first member of the list asks, the others follow.
    if (s.group?.participants[0] === s.selfName) this.shareQueue([sp.normalizeGuid(nextId)], 0, 0)
  }

  /** Makes `itemIds` the group's queue (what this member just started). */
  shareQueue(itemIds: readonly string[], index: number, startSeconds: number): void {
    if (itemIds.length === 0) return
    this.mark('queue')
    this.send((api) => sp.setNewQueue(api, itemIds, index, startSeconds * TICKS_PER_SECOND))
  }

  /** The video player started `itemId`: the group follows unless it is the group's own item. */
  videoItemStarted(itemId: string, startSeconds: number): void {
    if (!this.active) return
    const id = sp.normalizeGuid(itemId)
    if (playingEntry(useSyncPlay.getState().queue)?.itemId === id) return
    this.shareQueue([id], 0, startSeconds)
  }

  /** Shares what plays here (after creating a group, or "play" in an empty group). */
  private shareLocal(force = false): void {
    const v = this.videoPlayer
    const videoId = v?.itemId()
    if (v && videoId) {
      this.shareQueue([videoId], 0, v.position())
      return
    }
    const local = this.music?.localQueue()
    if (local && local.index >= 0 && local.ids.length && (local.playing || force)) {
      this.shareQueue(local.ids, local.index, this.music?.player.position() ?? 0)
    }
  }

  private readonly musicIntents: MusicIntents = {
    play: () => this.requestPlay(),
    pause: () => this.requestPause(),
    seek: (s) => this.requestSeek(s),
    next: () => this.requestNext(),
    previous: () => this.requestPrevious(),
    jump: (itemId) => {
      const entry = useSyncPlay.getState().queue?.playlist.find((e) => e.itemId === itemId)
      if (!entry) return
      this.mark('queue')
      this.send((api) => sp.setPlaylistItem(api, entry.playlistItemId))
    },
    newQueue: (ids, index, start) => this.shareQueue(ids, index, start),
  }

  // ----------------------------------------------------------------------- server events

  private onSocketStatus(status: SocketStatus): void {
    const phase = useSyncPlay.getState().phase
    if (status === 'reconnecting' && phase === 'joined') {
      // The server drops a session from its group when its socket closes: rejoin once back.
      useSyncPlay.setState({ phase: 'reconnecting' })
      this.notify({ kind: 'connectionLost' })
    } else if (status === 'open' && phase === 'reconnecting') {
      this.time?.restart()
      this.rejoin()
    }
  }

  private rejoin(): void {
    const group = useSyncPlay.getState().group
    if (!group) return
    this.rejoining = true
    // The room sends its queue again and waits for us: apply it even if it did not change.
    this.queueKey = ''
    this.send((api) => sp.joinGroup(api, group.id))
  }

  private onEvent(e: SyncPlayEvent): void {
    const group = useSyncPlay.getState().group
    const mine = (id: string) => !group || !id || id === group.id
    switch (e.type) {
      case 'groupJoined':
        return this.onGroupJoined(e.group)
      case 'groupLeft':
        if (this.leaving || !mine(e.groupId)) return
        this.teardown()
        this.notify({ kind: 'groupClosed' })
        return
      case 'userJoined':
      case 'userLeft': {
        if (!group || !mine(e.groupId)) return
        const participants = [...group.participants]
        if (e.type === 'userJoined') participants.push(e.user)
        else {
          const i = participants.indexOf(e.user)
          if (i >= 0) participants.splice(i, 1)
        }
        useSyncPlay.setState({ group: { ...group, participants } })
        this.refreshMembers()
        this.notify({ kind: e.type, name: e.user })
        return
      }
      case 'stateUpdate':
        if (mine(e.groupId)) this.onStateUpdate(e.state, e.reason)
        return
      case 'playQueue':
        if (mine(e.groupId)) void this.applyQueue(e.queue)
        return
      case 'command':
        if (mine(e.command.groupId)) this.applyCommand(e.command)
        return
      case 'error':
        return this.onError(e.kind)
    }
  }

  private onGroupJoined(group: SyncPlayGroup): void {
    const wasRejoin = this.rejoining
    this.rejoining = false
    useSyncPlay.setState({ phase: 'joined', group, groupState: group.state, sheetOpen: false })
    this.refreshMembers()
    this.notify(wasRejoin ? { kind: 'reconnected' } : { kind: 'joined', group: group.name })
    if (this.shareOnJoin) {
      this.shareOnJoin = false
      this.shareLocal()
    }
  }

  private onError(kind: SyncPlayErrorKind): void {
    const phase = useSyncPlay.getState().phase
    switch (kind) {
      case 'GroupDoesNotExist':
        if (this.rejoining && this.othersSeen === 0) {
          // We were alone: the group went away with our connection. Open it again.
          const name = useSyncPlay.getState().group?.name ?? ''
          useSyncPlay.setState({ queue: null, currentItem: null, currentKind: null })
          this.queueKey = ''
          this.shareOnJoin = true
          this.send((api) => sp.createGroup(api, name))
          return
        }
        this.teardown()
        this.notify({ kind: 'groupClosed' })
        return
      case 'NotInGroup':
        if (phase === 'joined' && !this.leaving && !this.rejoining) this.rejoin()
        return
      case 'CreateGroupDenied':
      case 'JoinGroupDenied':
        if (phase === 'connecting' || this.rejoining) this.teardown()
        this.notify({ kind: kind === 'CreateGroupDenied' ? 'createDenied' : 'joinDenied' })
        return
      case 'LibraryAccessDenied':
        if (phase === 'connecting') this.teardown()
        this.notify({ kind: 'libraryDenied' })
        return
    }
  }

  private refreshMembers(): void {
    const s = useSyncPlay.getState()
    if (!s.group) return
    const members = describeMembers(s.group.participants, s.selfName, s.selfReady, s.groupState)
    this.othersSeen = members.length - 1
    useSyncPlay.setState({ members })
  }

  private author(): string | null {
    const s = useSyncPlay.getState()
    return likelyAuthor(s.group?.participants ?? [], s.selfName)
  }

  private onStateUpdate(state: GroupState, reason: string): void {
    useSyncPlay.setState({ groupState: state })
    this.refreshMembers()
    if (state === 'Paused' && reason === 'Pause' && !this.isMine('pause')) {
      this.notify({ kind: 'paused', by: this.author() })
    } else if (state === 'Playing' && reason === 'Unpause' && !this.isMine('unpause')) {
      this.notify({ kind: 'resumed', by: this.author() })
    } else if (reason === 'Seek' && !this.isMine('seek')) {
      this.notify({ kind: 'seeked', by: this.author() })
    } else if (state === 'Waiting' && reason === 'Buffer' && useSyncPlay.getState().selfReady) {
      const now = Date.now()
      if (now - this.lastWaitingNotice > WAITING_NOTICE_GAP_MS) {
        this.lastWaitingNotice = now
        this.notify({ kind: 'waiting', name: waitingFor(useSyncPlay.getState().members) })
      }
    }
  }

  private async resolveItems(ids: readonly string[]): Promise<Map<string, BaseItemDto>> {
    const missing = [...new Set(ids)].filter((id) => !this.items.has(id))
    if (missing.length && this.api) {
      const found = await sp.fetchItemsByIds(this.api, this.userId, missing)
      found.forEach((item, id) => this.items.set(id, item))
    }
    return this.items
  }

  private async applyQueue(queue: PlayQueue): Promise<void> {
    const entry = playingEntry(queue)
    const before = playingEntry(useSyncPlay.getState().queue)
    const key = `${queue.lastUpdate}|${queue.playingItemIndex}|${queue.playlist.map((e) => e.playlistItemId).join(',')}`
    useSyncPlay.setState({ queue })
    if (key === this.queueKey) return
    const known = this.queueKey !== ''
    this.queueKey = key
    if (
      known &&
      entry &&
      before?.playlistItemId === entry.playlistItemId &&
      !QUEUE_RESETS.includes(queue.reason)
    ) {
      // Entries added, removed or moved around the one playing: playback goes on untouched.
      if (useSyncPlay.getState().currentKind === 'audio' && this.music) {
        try {
          const items = await this.resolveItems(queue.playlist.map((e) => e.itemId))
          if (key !== this.queueKey) return
          this.music.extend(playableOf(queue, items).tracks)
        } catch {
          /* the local queue just stays shorter */
        }
      }
      return
    }
    this.clearCommandTimer()
    this.allowPlay = false
    this.lastUnpause = null
    this.setRate(1)
    if (!entry) {
      useSyncPlay.setState({ currentItem: null, currentKind: null })
      return
    }
    let items: Map<string, BaseItemDto>
    try {
      items = await this.resolveItems(queue.playlist.map((e) => e.itemId))
    } catch {
      this.notify({ kind: 'error' })
      return
    }
    if (key !== this.queueKey) return // a newer queue arrived meanwhile
    const item = items.get(entry.itemId)
    if (!item) {
      // Not in this user's libraries: say so, and let the group go on without waiting for us.
      useSyncPlay.setState({ currentItem: null, currentKind: null })
      this.notify({ kind: 'itemUnavailable', title: null })
      this.setIgnoreWait(true)
      return
    }
    this.setIgnoreWait(false)
    const kind: MediaKind = item.MediaType === 'Video' ? 'video' : 'audio'
    useSyncPlay.setState({ currentItem: item, currentKind: kind })
    if (!this.isMine('queue') && QUEUE_RESETS.includes(queue.reason)) {
      this.notify({ kind: 'nowPlaying', title: itemTitle(item), by: this.author() })
    }
    const start = queue.startPositionTicks / TICKS_PER_SECOND

    if (kind === 'video') {
      // Music stops for the film; the video player opens on the group's item.
      if (this.music?.player.isPlaying()) this.music.player.pause()
      const v = this.videoPlayer
      if (v && v.itemId() === entry.itemId) this.align(start)
      else {
        this.awaitingReady = true
        this.reportedReady = null
        this.navigate?.(`/video/play/${item.Id}?start=${Math.round(queue.startPositionTicks)}`)
      }
    } else {
      const music = this.music
      if (!music) return
      if (this.videoPlayer) this.navigate?.('/music')
      const { tracks: playable, index: localIndex } = playableOf(queue, items)
      const local = music.localQueue()
      const ids = playable.map((t) => sp.normalizeGuid(t.Id))
      const same = local.ids.length === ids.length && local.ids.every((id, i) => id === ids[i])
      if (!same) music.load(playable, localIndex)
      else if (local.index !== localIndex) music.jump(localIndex)
      this.watch(music.player)
      this.align(start)
    }
    const pending = this.pendingCommand
    this.pendingCommand = null
    if (pending && pending.playlistItemId === entry.playlistItemId) this.applyCommand(pending)
  }

  private setIgnoreWait(ignore: boolean): void {
    if (this.ignoringWait === ignore) return
    this.ignoringWait = ignore
    this.send((api) => sp.setIgnoreWait(api, ignore))
  }

  // ----------------------------------------------------------------------- commands

  private clearCommandTimer(): void {
    if (this.commandTimer) clearTimeout(this.commandTimer)
    this.commandTimer = null
  }

  private at(localMs: number, run: () => void): void {
    this.clearCommandTimer()
    const delay = localMs - Date.now()
    if (delay <= 4) run()
    else this.commandTimer = setTimeout(run, delay)
  }

  private applyCommand(cmd: SyncPlayCommand): void {
    if (cmd.command !== 'Unpause') {
      // Unpause is re-sent to whoever reports Ready again; re-applying it is how we resync.
      const key = commandKey(cmd)
      if (this.seen.includes(key)) return
      this.seen = [...this.seen.slice(-19), key]
    }
    const entry = playingEntry(useSyncPlay.getState().queue)
    if (cmd.command === 'Stop') {
      this.clearCommandTimer()
      this.allowPlay = false
      this.lastUnpause = null
      this.setRate(1)
      // The Stop a new group sends before anything is shared does not stop what we play.
      if (cmd.playlistItemId === sp.EMPTY_GUID && this.shareOnJoin) return
      this.player?.pause()
      return
    }
    if (!entry || cmd.playlistItemId !== entry.playlistItemId) {
      // For a queue we have not applied yet (messages can cross): keep it for then.
      this.pendingCommand = cmd
      return
    }
    const p = this.player
    const offset = this.time?.offset ?? 0
    const plan = planCommand(cmd, Date.now(), offset)
    switch (cmd.command) {
      case 'Unpause': {
        this.lastUnpause = { when: cmd.when, positionTicks: cmd.positionTicks }
        this.awaitingReady = false
        if (
          p &&
          !plan.late &&
          !p.isPlaying() &&
          Math.abs(p.position() - plan.positionSeconds) > 0.05
        ) {
          p.seek(plan.positionSeconds)
        }
        this.at(plan.atLocal, () => {
          this.allowPlay = true
          this.graceUntil = Date.now() + GRACE_MS
          const cur = this.player
          if (!cur) return
          const target = expectedPosition(cmd, this.time?.serverNow() ?? Date.now() + offset)
          const off = Math.abs(cur.position() - target)
          if (off > (cur.isPlaying() ? 0.3 : 0.05)) cur.seek(target)
          if (!cur.isPlaying()) cur.play()
        })
        return
      }
      case 'Pause':
        this.at(plan.atLocal, () => {
          this.allowPlay = false
          this.lastUnpause = null
          this.setRate(1)
          const cur = this.player
          if (!cur) return
          cur.pause()
          if (Math.abs(cur.position() - plan.positionSeconds) > 0.05) cur.seek(plan.positionSeconds)
        })
        return
      case 'Seek':
        this.clearCommandTimer()
        this.align(plan.positionSeconds)
        return
    }
  }
}

/** Queue updates that change what plays (the others only edit the list around it). */
const QUEUE_RESETS = ['NewPlaylist', 'SetCurrentItem', 'NextItem', 'PreviousItem']

/** The entries this user can access (others are left out of the local queue), and the current one. */
function playableOf(
  queue: PlayQueue,
  items: Map<string, BaseItemDto>,
): { tracks: BaseItemDto[]; index: number } {
  const tracks: BaseItemDto[] = []
  let index = 0
  queue.playlist.forEach((e, i) => {
    const it = items.get(e.itemId)
    if (!it) return
    if (i === queue.playingItemIndex) index = tracks.length
    tracks.push(it)
  })
  return { tracks, index }
}

function itemTitle(item: BaseItemDto): string {
  if (item.Type === 'Episode' && item.SeriesName) return `${item.SeriesName} · ${item.Name ?? ''}`
  if (item.Type === 'Audio' && item.Artists?.length)
    return `${item.Name ?? ''} · ${item.Artists[0]}`
  return item.Name ?? ''
}

export const syncPlayManager = new SyncPlayManager()
