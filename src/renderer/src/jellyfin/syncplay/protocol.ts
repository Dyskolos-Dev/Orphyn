/**
 * Jellyfin SyncPlay (server 10.9+): the REST requests a member sends and the WebSocket messages
 * (`SyncPlayCommand`, `SyncPlayGroupUpdate`) the server pushes, parsed into plain typed events.
 * Shapes checked against Jellyfin 10.11. See docs/SYNCPLAY.md.
 */
import type { Api } from '@jellyfin/sdk'
import { jfRequest } from '../client'
import type { SocketMessage } from './socket'

export type GroupState = 'Idle' | 'Waiting' | 'Paused' | 'Playing'
export type SyncPlayCommandKind = 'Unpause' | 'Pause' | 'Stop' | 'Seek'

export interface SyncPlayGroup {
  id: string
  name: string
  state: GroupState
  /** User names (the server gives no ids). */
  participants: string[]
  lastUpdatedAt: number
}

export interface SyncPlayCommand {
  groupId: string
  playlistItemId: string
  /** Server time (ms since epoch) at which the command takes effect. */
  when: number
  positionTicks: number
  command: SyncPlayCommandKind
  emittedAt: number
}

export interface QueueEntry {
  itemId: string
  playlistItemId: string
}

export interface PlayQueue {
  reason: string
  lastUpdate: number
  playlist: QueueEntry[]
  playingItemIndex: number
  startPositionTicks: number
  isPlaying: boolean
  shuffleMode: string
  repeatMode: string
}

export type SyncPlayErrorKind =
  | 'NotInGroup'
  | 'GroupDoesNotExist'
  | 'CreateGroupDenied'
  | 'JoinGroupDenied'
  | 'LibraryAccessDenied'

export type SyncPlayEvent =
  | { type: 'command'; command: SyncPlayCommand }
  | { type: 'groupJoined'; group: SyncPlayGroup }
  | { type: 'groupLeft'; groupId: string }
  | { type: 'userJoined'; groupId: string; user: string }
  | { type: 'userLeft'; groupId: string; user: string }
  | { type: 'stateUpdate'; groupId: string; state: GroupState; reason: string }
  | { type: 'playQueue'; groupId: string; queue: PlayQueue }
  | { type: 'error'; groupId: string; kind: SyncPlayErrorKind }

export const EMPTY_GUID = '00000000000000000000000000000000'

/** Guids come with and without dashes depending on the message: compare them normalised. */
export function normalizeGuid(id: unknown): string {
  return typeof id === 'string' ? id.replace(/-/g, '').toLowerCase() : ''
}

/** Parses a .NET date (`2026-11-17T18:27:41.0429711Z`: 7 fractional digits) to ms since epoch. */
export function parseServerDate(value: unknown): number {
  if (typeof value !== 'string') return NaN
  const trimmed = value.replace(/(\.\d{3})\d+/, '$1')
  return Date.parse(trimmed)
}

/** ms since epoch to the ISO string the server expects. */
export const toServerDate = (ms: number): string => new Date(ms).toISOString()

type Obj = Record<string, unknown>
const isObj = (v: unknown): v is Obj => typeof v === 'object' && v !== null && !Array.isArray(v)
const num = (v: unknown, fallback = 0): number =>
  typeof v === 'number' && Number.isFinite(v) ? v : fallback
const str = (v: unknown): string => (typeof v === 'string' ? v : '')

const GROUP_STATES: readonly GroupState[] = ['Idle', 'Waiting', 'Paused', 'Playing']
const COMMANDS: readonly SyncPlayCommandKind[] = ['Unpause', 'Pause', 'Stop', 'Seek']
const ERRORS: readonly SyncPlayErrorKind[] = [
  'NotInGroup',
  'GroupDoesNotExist',
  'CreateGroupDenied',
  'JoinGroupDenied',
  'LibraryAccessDenied',
]

function toState(v: unknown): GroupState {
  return GROUP_STATES.includes(v as GroupState) ? (v as GroupState) : 'Idle'
}

export function parseGroup(raw: unknown): SyncPlayGroup | null {
  if (!isObj(raw) || !raw.GroupId) return null
  return {
    id: normalizeGuid(raw.GroupId),
    name: str(raw.GroupName),
    state: toState(raw.State),
    participants: Array.isArray(raw.Participants)
      ? raw.Participants.filter((p): p is string => typeof p === 'string')
      : [],
    lastUpdatedAt: parseServerDate(raw.LastUpdatedAt) || 0,
  }
}

export function parseCommand(raw: unknown): SyncPlayCommand | null {
  if (!isObj(raw) || !COMMANDS.includes(raw.Command as SyncPlayCommandKind)) return null
  const when = parseServerDate(raw.When)
  if (!Number.isFinite(when)) return null
  return {
    groupId: normalizeGuid(raw.GroupId),
    playlistItemId: normalizeGuid(raw.PlaylistItemId),
    when,
    positionTicks: num(raw.PositionTicks),
    command: raw.Command as SyncPlayCommandKind,
    emittedAt: parseServerDate(raw.EmittedAt) || when,
  }
}

export function parsePlayQueue(raw: unknown): PlayQueue | null {
  if (!isObj(raw) || !Array.isArray(raw.Playlist)) return null
  const playlist: QueueEntry[] = []
  for (const e of raw.Playlist) {
    if (isObj(e) && e.ItemId && e.PlaylistItemId) {
      playlist.push({
        itemId: normalizeGuid(e.ItemId),
        playlistItemId: normalizeGuid(e.PlaylistItemId),
      })
    }
  }
  return {
    reason: str(raw.Reason),
    lastUpdate: parseServerDate(raw.LastUpdate) || 0,
    playlist,
    playingItemIndex: num(raw.PlayingItemIndex, -1),
    startPositionTicks: num(raw.StartPositionTicks),
    isPlaying: raw.IsPlaying === true,
    shuffleMode: str(raw.ShuffleMode),
    repeatMode: str(raw.RepeatMode),
  }
}

/** One socket message to a SyncPlay event; null for anything else (keep-alives, other features). */
export function parseSyncPlayMessage(msg: SocketMessage): SyncPlayEvent | null {
  if (msg.MessageType === 'SyncPlayCommand') {
    const command = parseCommand(msg.Data)
    return command ? { type: 'command', command } : null
  }
  if (msg.MessageType !== 'SyncPlayGroupUpdate' || !isObj(msg.Data)) return null
  const update = msg.Data
  const groupId = normalizeGuid(update.GroupId)
  const data = update.Data
  switch (update.Type) {
    case 'GroupJoined': {
      const group = parseGroup(data)
      return group ? { type: 'groupJoined', group } : null
    }
    case 'GroupLeft':
      return { type: 'groupLeft', groupId: normalizeGuid(data) || groupId }
    case 'UserJoined':
      return { type: 'userJoined', groupId, user: str(data) }
    case 'UserLeft':
      return { type: 'userLeft', groupId, user: str(data) }
    case 'StateUpdate':
      if (!isObj(data)) return null
      return { type: 'stateUpdate', groupId, state: toState(data.State), reason: str(data.Reason) }
    case 'PlayQueue': {
      const queue = parsePlayQueue(data)
      return queue ? { type: 'playQueue', groupId, queue } : null
    }
    default:
      if (ERRORS.includes(update.Type as SyncPlayErrorKind)) {
        return { type: 'error', groupId, kind: update.Type as SyncPlayErrorKind }
      }
      return null
  }
}

// ---------------------------------------------------------------------------------- REST

const post = (api: Api, path: string, data?: unknown) =>
  jfRequest<void>(api, 'POST', `/SyncPlay/${path}`, { data: data ?? {} })

export async function listGroups(api: Api): Promise<SyncPlayGroup[]> {
  const raw = await jfRequest<unknown>(api, 'GET', '/SyncPlay/List')
  return Array.isArray(raw) ? raw.map(parseGroup).filter((g): g is SyncPlayGroup => g !== null) : []
}

export const createGroup = (api: Api, name: string) => post(api, 'New', { GroupName: name })
export const joinGroup = (api: Api, groupId: string) => post(api, 'Join', { GroupId: groupId })
export const leaveGroup = (api: Api) => post(api, 'Leave')

export const setNewQueue = (
  api: Api,
  itemIds: readonly string[],
  playingIndex: number,
  startPositionTicks: number,
) =>
  post(api, 'SetNewQueue', {
    PlayingQueue: itemIds,
    PlayingItemPosition: playingIndex,
    StartPositionTicks: Math.max(0, Math.round(startPositionTicks)),
  })

export const queueItems = (api: Api, itemIds: readonly string[], mode: 'Queue' | 'QueueNext') =>
  post(api, 'Queue', { ItemIds: itemIds, Mode: mode })
export const setPlaylistItem = (api: Api, playlistItemId: string) =>
  post(api, 'SetPlaylistItem', { PlaylistItemId: playlistItemId })
/** The server ignores it unless `current` is still the playing entry: duplicates are harmless. */
export const nextItem = (api: Api, current: string) =>
  post(api, 'NextItem', { PlaylistItemId: current })
export const previousItem = (api: Api, current: string) =>
  post(api, 'PreviousItem', { PlaylistItemId: current })

/** With `true` the group stops waiting for this member (it cannot play the item). */
export const setIgnoreWait = (api: Api, ignore: boolean) =>
  post(api, 'SetIgnoreWait', { IgnoreWait: ignore })

export const requestUnpause = (api: Api) => post(api, 'Unpause')
export const requestPause = (api: Api) => post(api, 'Pause')
export const requestStop = (api: Api) => post(api, 'Stop')
export const requestSeek = (api: Api, positionTicks: number) =>
  post(api, 'Seek', { PositionTicks: Math.max(0, Math.round(positionTicks)) })

export interface BufferingReport {
  /** Server time (ms). */
  when: number
  positionTicks: number
  isPlaying: boolean
  playlistItemId: string
}

const bufferingBody = (r: BufferingReport) => ({
  When: toServerDate(r.when),
  PositionTicks: Math.max(0, Math.round(r.positionTicks)),
  IsPlaying: r.isPlaying,
  PlaylistItemId: r.playlistItemId,
})

export const reportBuffering = (api: Api, r: BufferingReport) =>
  post(api, 'Buffering', bufferingBody(r))
export const reportReady = (api: Api, r: BufferingReport) => post(api, 'Ready', bufferingBody(r))
/** Tells the server our round-trip time (ms); it uses it to place `When` far enough ahead. */
export const sendPing = (api: Api, pingMs: number) =>
  post(api, 'Ping', { Ping: Math.max(0, Math.round(pingMs)) })

/** `GET /GetUtcTime`: when the server received the request and sent the answer (ms). */
export async function getUtcTime(api: Api): Promise<{ received: number; sent: number }> {
  const raw = await jfRequest<unknown>(api, 'GET', '/GetUtcTime')
  if (!isObj(raw)) throw new Error('Bad /GetUtcTime answer')
  const received = parseServerDate(raw.RequestReceptionTime)
  const sent = parseServerDate(raw.ResponseTransmissionTime)
  if (!Number.isFinite(received) || !Number.isFinite(sent))
    throw new Error('Bad /GetUtcTime answer')
  return { received, sent }
}
