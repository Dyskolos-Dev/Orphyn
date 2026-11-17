import { describe, expect, it } from 'vitest'
import { createJellyfinApi } from '../client'
import { fakeHttp } from '../test-utils'
import {
  getUtcTime,
  listGroups,
  normalizeGuid,
  parseServerDate,
  parseSyncPlayMessage,
  reportReady,
  requestSeek,
  setNewQueue,
} from './protocol'

// Messages as a Jellyfin 10.11 server sends them (recorded).
const groupJoined = {
  MessageId: 'x',
  MessageType: 'SyncPlayGroupUpdate',
  Data: {
    GroupId: '819183e687614dcbbfc7c2d2a61ed257',
    Type: 'GroupJoined',
    Data: {
      GroupId: '819183e687614dcbbfc7c2d2a61ed257',
      GroupName: 'Movie night',
      State: 'Idle',
      Participants: ['alex'],
      LastUpdatedAt: '2026-11-17T18:25:57.8246537Z',
    },
  },
}

const unpause = {
  MessageType: 'SyncPlayCommand',
  Data: {
    GroupId: '4f94f2af67b14bfbb2aef5389e5ba636',
    PlaylistItemId: '08008fbd2c83454f85bc99445c55f706',
    When: '2026-11-17T18:27:41.0429711Z',
    PositionTicks: 135783670,
    Command: 'Unpause',
    EmittedAt: '2026-11-17T18:27:40.0429711Z',
  },
}

const playQueue = {
  MessageType: 'SyncPlayGroupUpdate',
  Data: {
    GroupId: 'G1',
    Type: 'PlayQueue',
    Data: {
      Reason: 'NewPlaylist',
      LastUpdate: '2026-11-17T18:25:59.1717244Z',
      Playlist: [
        { ItemId: 'DC0508752A83A30648CD20A0B380984F', PlaylistItemId: 'd522-83a3' },
        { ItemId: 'f72d52fb701195cd3ebe79d945fcb6fa', PlaylistItemId: '46f8' },
      ],
      PlayingItemIndex: 1,
      StartPositionTicks: 50_000_000,
      IsPlaying: true,
      ShuffleMode: 'Sorted',
      RepeatMode: 'RepeatNone',
    },
  },
}

describe('SyncPlay message parsing', () => {
  it('parses GroupJoined', () => {
    expect(parseSyncPlayMessage(groupJoined)).toEqual({
      type: 'groupJoined',
      group: {
        id: '819183e687614dcbbfc7c2d2a61ed257',
        name: 'Movie night',
        state: 'Idle',
        participants: ['alex'],
        lastUpdatedAt: Date.parse('2026-11-17T18:25:57.824Z'),
      },
    })
  })

  it('parses a command with 7-digit fractions', () => {
    const e = parseSyncPlayMessage(unpause)
    expect(e).toEqual({
      type: 'command',
      command: {
        groupId: '4f94f2af67b14bfbb2aef5389e5ba636',
        playlistItemId: '08008fbd2c83454f85bc99445c55f706',
        when: Date.parse('2026-11-17T18:27:41.042Z'),
        positionTicks: 135783670,
        command: 'Unpause',
        emittedAt: Date.parse('2026-11-17T18:27:40.042Z'),
      },
    })
  })

  it('parses a play queue and normalises guids', () => {
    const e = parseSyncPlayMessage(playQueue)
    expect(e?.type).toBe('playQueue')
    if (e?.type !== 'playQueue') return
    expect(e.groupId).toBe('g1')
    expect(e.queue.playlist).toEqual([
      { itemId: 'dc0508752a83a30648cd20a0b380984f', playlistItemId: 'd52283a3' },
      { itemId: 'f72d52fb701195cd3ebe79d945fcb6fa', playlistItemId: '46f8' },
    ])
    expect(e.queue).toMatchObject({
      reason: 'NewPlaylist',
      playingItemIndex: 1,
      startPositionTicks: 50_000_000,
      isPlaying: true,
    })
  })

  it('parses the small updates and the errors', () => {
    const upd = (Type: string, Data: unknown) => ({
      MessageType: 'SyncPlayGroupUpdate',
      Data: { GroupId: 'abc', Type, Data },
    })
    expect(parseSyncPlayMessage(upd('UserJoined', 'sam'))).toEqual({
      type: 'userJoined',
      groupId: 'abc',
      user: 'sam',
    })
    expect(parseSyncPlayMessage(upd('UserLeft', 'sam'))).toMatchObject({ type: 'userLeft' })
    expect(
      parseSyncPlayMessage(upd('StateUpdate', { State: 'Waiting', Reason: 'Buffer' })),
    ).toEqual({ type: 'stateUpdate', groupId: 'abc', state: 'Waiting', reason: 'Buffer' })
    // GroupLeft carries the id with dashes.
    expect(parseSyncPlayMessage(upd('GroupLeft', '4f94f2af-67b1-4bfb-b2ae-f5389e5ba636'))).toEqual({
      type: 'groupLeft',
      groupId: '4f94f2af67b14bfbb2aef5389e5ba636',
    })
    expect(parseSyncPlayMessage(upd('GroupDoesNotExist', ''))).toEqual({
      type: 'error',
      groupId: 'abc',
      kind: 'GroupDoesNotExist',
    })
  })

  it('ignores other messages and malformed ones', () => {
    expect(parseSyncPlayMessage({ MessageType: 'KeepAlive' })).toBeNull()
    expect(parseSyncPlayMessage({ MessageType: 'ForceKeepAlive', Data: 60 })).toBeNull()
    expect(
      parseSyncPlayMessage({ MessageType: 'SyncPlayCommand', Data: { Command: 'Dance' } }),
    ).toBeNull()
    expect(
      parseSyncPlayMessage({
        MessageType: 'SyncPlayCommand',
        Data: { Command: 'Pause', When: 'x' },
      }),
    ).toBeNull()
    expect(
      parseSyncPlayMessage({ MessageType: 'SyncPlayGroupUpdate', Data: { Type: 'Whatever' } }),
    ).toBeNull()
  })

  it('normalises dates and guids', () => {
    expect(parseServerDate('2026-11-17T18:27:41.0429711Z')).toBe(
      Date.parse('2026-11-17T18:27:41.042Z'),
    )
    expect(parseServerDate('2026-11-17T18:27:41Z')).toBe(Date.parse('2026-11-17T18:27:41Z'))
    expect(parseServerDate(null)).toBeNaN()
    expect(normalizeGuid('4F94F2AF-67B1-4BFB-B2AE-F5389E5BA636')).toBe(
      '4f94f2af67b14bfbb2aef5389e5ba636',
    )
  })
})

describe('SyncPlay REST', () => {
  function setup(reply: unknown = undefined) {
    const fake = fakeHttp(() => ({ status: reply === undefined ? 204 : 200, data: reply }))
    const api = createJellyfinApi({
      baseUrl: 'https://jf.example',
      deviceId: 'dev',
      accessToken: 'tok',
      http: fake.http,
    })
    return { api, calls: fake.calls }
  }

  it('sends the bodies the server expects', async () => {
    const { api, calls } = setup()
    await setNewQueue(api, ['a', 'b'], 1, 12.4)
    await requestSeek(api, 300_000_000.6)
    await reportReady(api, {
      when: Date.parse('2026-11-17T18:00:00.000Z'),
      positionTicks: 10,
      isPlaying: true,
      playlistItemId: 'p1',
    })
    expect(calls.map((c) => [c.method, c.url.replace('https://jf.example', ''), c.data])).toEqual([
      [
        'POST',
        '/SyncPlay/SetNewQueue',
        { PlayingQueue: ['a', 'b'], PlayingItemPosition: 1, StartPositionTicks: 12 },
      ],
      ['POST', '/SyncPlay/Seek', { PositionTicks: 300_000_001 }],
      [
        'POST',
        '/SyncPlay/Ready',
        {
          When: '2026-11-17T18:00:00.000Z',
          PositionTicks: 10,
          IsPlaying: true,
          PlaylistItemId: 'p1',
        },
      ],
    ])
  })

  it('lists groups and reads the server time', async () => {
    const list = setup([
      { GroupId: 'G', GroupName: 'Room', State: 'Playing', Participants: ['a', 'b'] },
      { nope: true },
    ])
    expect(await listGroups(list.api)).toEqual([
      { id: 'g', name: 'Room', state: 'Playing', participants: ['a', 'b'], lastUpdatedAt: 0 },
    ])
    const utc = setup({
      RequestReceptionTime: '2026-11-17T18:25:57.300187Z',
      ResponseTransmissionTime: '2026-11-17T18:25:57.3015Z',
    })
    expect(await getUtcTime(utc.api)).toEqual({
      received: Date.parse('2026-11-17T18:25:57.300Z'),
      sent: Date.parse('2026-11-17T18:25:57.301Z'),
    })
  })
})
