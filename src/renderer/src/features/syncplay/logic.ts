/**
 * Pure SyncPlay decisions (no I/O): when and where a group command lands on this computer, how to
 * correct drift while playing, and what can be told about the other members.
 */
import type { GroupState, QueueEntry, SyncPlayCommand } from '@renderer/jellyfin'

export const TICKS_PER_SECOND = 10_000_000

// ------------------------------------------------------------------------- scheduling

export interface CommandPlan {
  /** Local time (ms) at which to act. */
  atLocal: number
  /** Position (s) to be at when acting (for Unpause: where playback starts at `atLocal`). */
  positionSeconds: number
  /** The command's moment has already passed (we act at once, adjusted for the lateness). */
  late: boolean
}

/**
 * Where and when a command applies here. `offsetMs` is server − local (see TimeSync).
 * - Unpause: start at `When` from `PositionTicks`; when late, start now, further in.
 * - Pause: stop at `When`, then sit on `PositionTicks` (where everyone stops).
 * - Seek / Stop: now.
 */
export function planCommand(cmd: SyncPlayCommand, nowLocal: number, offsetMs: number): CommandPlan {
  const whenLocal = cmd.when - offsetMs
  const position = cmd.positionTicks / TICKS_PER_SECOND
  switch (cmd.command) {
    case 'Unpause': {
      if (whenLocal >= nowLocal)
        return { atLocal: whenLocal, positionSeconds: position, late: false }
      return {
        atLocal: nowLocal,
        positionSeconds: position + (nowLocal - whenLocal) / 1000,
        late: true,
      }
    }
    case 'Pause':
      return {
        atLocal: Math.max(whenLocal, nowLocal),
        positionSeconds: position,
        late: whenLocal < nowLocal,
      }
    default:
      return { atLocal: nowLocal, positionSeconds: position, late: whenLocal < nowLocal }
  }
}

/** The server repeats a command to whoever reports Ready again: apply each one once. */
export function commandKey(cmd: SyncPlayCommand): string {
  return `${cmd.command}|${cmd.playlistItemId}|${cmd.when}|${cmd.positionTicks}`
}

/** Where the group is now (s), given the last Unpause and the server clock. */
export function expectedPosition(
  lastUnpause: { when: number; positionTicks: number },
  serverNow: number,
): number {
  return (
    lastUnpause.positionTicks / TICKS_PER_SECOND + Math.max(0, serverNow - lastUnpause.when) / 1000
  )
}

// ------------------------------------------------------------------------- drift correction

export type DriftAction = { kind: 'none' } | { kind: 'rate'; rate: number } | { kind: 'seek' }

export const DRIFT = {
  /** Below this, playback is in sync: normal speed. */
  deadBandS: 0.04,
  /** Up to this, a player that can change speed nudges it; beyond, it seeks. */
  nudgeLimitS: 1,
  /** A player without speed control (music output) leaves this much alone, then seeks. */
  seekOnlyThresholdS: 0.5,
  minRate: 0.95,
  maxRate: 1.05,
  /** The nudge aims to absorb the drift over about this long (s). */
  correctionWindowS: 2,
}

/**
 * `driftS` = local position − expected position (positive: we are ahead). Small drift → a gentle
 * playbackRate change (0.95–1.05) that absorbs it in a few seconds without a sound glitch; large
 * drift → a seek.
 */
export function driftAction(driftS: number, canNudge: boolean): DriftAction {
  const abs = Math.abs(driftS)
  if (!Number.isFinite(driftS)) return { kind: 'none' }
  if (!canNudge) return abs > DRIFT.seekOnlyThresholdS ? { kind: 'seek' } : { kind: 'none' }
  if (abs <= DRIFT.deadBandS) return { kind: 'rate', rate: 1 }
  if (abs > DRIFT.nudgeLimitS) return { kind: 'seek' }
  const raw = 1 - driftS / DRIFT.correctionWindowS
  const rate = Math.min(DRIFT.maxRate, Math.max(DRIFT.minRate, raw))
  return { kind: 'rate', rate: Math.round(rate * 1000) / 1000 }
}

// ------------------------------------------------------------------------- queue

/** Same items in the same order (the local queue already matches the group's). */
export function sameItems(local: readonly string[], group: readonly QueueEntry[]): boolean {
  return local.length === group.length && local.every((id, i) => id === group[i]?.itemId)
}

/** What a local change of the current entry means for the group. */
export function queueMove(
  from: number,
  to: number,
  length: number,
): 'next' | 'previous' | 'jump' | 'none' {
  if (from === to) return 'none'
  if (to === from + 1 || (from === length - 1 && to === 0)) return 'next'
  if (to === from - 1) return 'previous'
  return 'jump'
}

// ------------------------------------------------------------------------- members

export type MemberState = 'ready' | 'buffering' | 'waiting'

export interface Member {
  name: string
  isSelf: boolean
  state: MemberState
}

/**
 * The server only gives the members' names and the group's state, not who is buffering. What we
 * do know: our own state; when the group waits while we are ready and only one other person is
 * there, it is that person; otherwise the others are shown as "waiting" without a name.
 */
export function describeMembers(
  participants: readonly string[],
  selfName: string,
  selfReady: boolean,
  groupState: GroupState,
): Member[] {
  let selfSeen = false
  const others = participants.filter((p) => {
    if (!selfSeen && p === selfName) {
      selfSeen = true
      return false
    }
    return true
  })
  const othersState: MemberState =
    groupState === 'Waiting'
      ? selfReady && others.length === 1
        ? 'buffering'
        : 'waiting'
      : 'ready'
  return [
    { name: selfName, isSelf: true, state: selfReady ? 'ready' : 'buffering' },
    ...others.map((name) => ({ name, isSelf: false, state: othersState })),
  ]
}

/** Who the group waits for, when we can tell (we are ready and there is one other member). */
export function waitingFor(members: readonly Member[]): string | null {
  const buffering = members.filter((m) => !m.isSelf && m.state === 'buffering')
  return buffering.length === 1 ? (buffering[0]?.name ?? null) : null
}

/**
 * Who issued a command we did not send ourselves: only knowable when there is exactly one other
 * member. Returns null otherwise (the UI then says "the group").
 */
export function likelyAuthor(participants: readonly string[], selfName: string): string | null {
  const others = participants.filter((p) => p !== selfName)
  return others.length === 1 ? (others[0] ?? null) : null
}

/** Two-letter initials for an avatar. */
export function memberInitials(name: string): string {
  const parts = name
    .trim()
    .split(/[\s._-]+/)
    .filter(Boolean)
  const [first = '', second = ''] = parts
  if (!first) return '?'
  if (!second) return first.slice(0, 2).toUpperCase()
  return (first.charAt(0) + second.charAt(0)).toUpperCase()
}

const AVATAR_COLORS = ['blue', 'teal', 'green', 'orange', 'pink', 'purple', 'indigo', 'red']

/** Stable system colour token for a member's avatar, from their name. */
export function memberColor(name: string): string {
  let h = 0
  for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) >>> 0
  return `var(--system-${AVATAR_COLORS[h % AVATAR_COLORS.length]})`
}
