import { afterEach, describe, expect, it, vi } from 'vitest'
import { JellyfinSocket, backoffDelay, socketUrl, type WebSocketLike } from './socket'

class FakeSocket implements WebSocketLike {
  readyState = 0
  onopen: ((ev: unknown) => void) | null = null
  onclose: ((ev: unknown) => void) | null = null
  onerror: ((ev: unknown) => void) | null = null
  onmessage: ((ev: { data: unknown }) => void) | null = null
  sent: string[] = []
  closed = false
  constructor(readonly url: string) {}
  send(data: string) {
    this.sent.push(data)
  }
  close() {
    this.closed = true
  }
  open() {
    this.readyState = 1
    this.onopen?.({})
  }
  receive(msg: unknown) {
    this.onmessage?.({ data: JSON.stringify(msg) })
  }
  drop() {
    this.readyState = 3
    this.onclose?.({})
  }
}

function setup() {
  const sockets: FakeSocket[] = []
  const socket = new JellyfinSocket('wss://jf/socket', {
    createSocket: (url) => {
      const s = new FakeSocket(url)
      sockets.push(s)
      return s
    },
    random: () => 0.5,
  })
  return { socket, sockets }
}

describe('socketUrl / backoff', () => {
  it('builds the socket address from the server address', () => {
    expect(socketUrl('https://jf.example.com/jellyfin/', 'tok', 'dev 1')).toBe(
      'wss://jf.example.com/jellyfin/socket?api_key=tok&deviceId=dev+1',
    )
    expect(socketUrl('http://192.168.1.2:8096', 't', 'd')).toBe(
      'ws://192.168.1.2:8096/socket?api_key=t&deviceId=d',
    )
  })

  it('backs off exponentially with jitter, capped', () => {
    expect(backoffDelay(0, () => 0.5)).toBe(1000)
    expect(backoffDelay(3, () => 0.5)).toBe(8000)
    expect(backoffDelay(20, () => 0.5)).toBe(30_000)
    expect(backoffDelay(0, () => 0)).toBe(800)
    expect(backoffDelay(0, () => 1)).toBe(1200)
  })
})

describe('JellyfinSocket', () => {
  afterEach(() => vi.useRealTimers())

  it('delivers messages and keeps alive at half the server timeout', () => {
    vi.useFakeTimers()
    const { socket, sockets } = setup()
    const got: string[] = []
    socket.onMessage((m) => got.push(m.MessageType))
    socket.connect()
    const ws = sockets[0]!
    ws.open()
    expect(socket.status).toBe('open')
    ws.receive({ MessageType: 'ForceKeepAlive', Data: 60 })
    expect(ws.sent).toEqual(['{"MessageType":"KeepAlive"}'])
    vi.advanceTimersByTime(29_000)
    ws.receive({ MessageType: 'KeepAlive' })
    vi.advanceTimersByTime(1000)
    expect(ws.sent).toHaveLength(2)
    ws.receive({ MessageType: 'SyncPlayCommand', Data: {} })
    ws.onmessage?.({ data: 'not json' })
    expect(got).toEqual(['ForceKeepAlive', 'KeepAlive', 'SyncPlayCommand'])
    socket.close()
  })

  it('reconnects with backoff after a drop, and stops when closed', () => {
    vi.useFakeTimers()
    const { socket, sockets } = setup()
    const statuses: string[] = []
    socket.onStatus((s) => statuses.push(s))
    socket.connect()
    sockets[0]!.open()
    sockets[0]!.drop()
    expect(socket.status).toBe('reconnecting')
    vi.advanceTimersByTime(999)
    expect(sockets).toHaveLength(1)
    vi.advanceTimersByTime(1)
    expect(sockets).toHaveLength(2)
    sockets[1]!.drop() // failed attempt: next wait is longer
    vi.advanceTimersByTime(1999)
    expect(sockets).toHaveLength(2)
    vi.advanceTimersByTime(1)
    sockets[2]!.open()
    expect(statuses).toEqual(['connecting', 'open', 'reconnecting', 'open'])
    socket.close()
    expect(sockets[2]!.closed).toBe(true)
    vi.advanceTimersByTime(60_000)
    expect(sockets).toHaveLength(3)
    expect(socket.status).toBe('closed')
  })

  it('notices a silent dead link', () => {
    vi.useFakeTimers()
    const { socket, sockets } = setup()
    socket.connect()
    sockets[0]!.open()
    sockets[0]!.receive({ MessageType: 'ForceKeepAlive', Data: 10 }) // keep-alive every 5 s
    vi.advanceTimersByTime(15_000) // nothing heard back
    expect(sockets[0]!.closed).toBe(true)
    expect(socket.status).toBe('reconnecting')
    socket.close()
  })

  it('whenOpen resolves on open', async () => {
    const { socket, sockets } = setup()
    socket.connect()
    const opened = socket.whenOpen()
    sockets[0]!.open()
    await expect(opened).resolves.toBeUndefined()
    socket.close()
  })
})
