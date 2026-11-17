/**
 * The server WebSocket (`/socket?api_key=…&deviceId=…`): keep-alive as the server asks
 * (`ForceKeepAlive`), a watchdog that notices a silent dead connection, and reconnection with an
 * exponential backoff. SyncPlay is its only user for now.
 */

export interface SocketMessage {
  MessageType: string
  MessageId?: string
  Data?: unknown
}

export type SocketStatus = 'idle' | 'connecting' | 'open' | 'reconnecting' | 'closed'

/** The part of `WebSocket` the client needs (tests pass a fake). */
export interface WebSocketLike {
  readonly readyState: number
  onopen: ((ev: unknown) => void) | null
  onclose: ((ev: unknown) => void) | null
  onerror: ((ev: unknown) => void) | null
  onmessage: ((ev: { data: unknown }) => void) | null
  send(data: string): void
  close(code?: number, reason?: string): void
}

const OPEN = 1

/** `wss://host/base/socket?api_key=TOKEN&deviceId=ID` from the server's http(s) address. */
export function socketUrl(baseUrl: string, token: string, deviceId: string): string {
  const url = new URL(baseUrl.replace(/\/+$/, '') + '/socket')
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:'
  // `api_key` is what the socket endpoint reads (checked on 10.11), `ApiKey` is not accepted there.
  url.searchParams.set('api_key', token)
  url.searchParams.set('deviceId', deviceId)
  return url.toString()
}

/** 1 s, 2 s, 4 s … capped at 30 s, with ±20 % jitter so clients do not reconnect in lockstep. */
export function backoffDelay(attempt: number, random: () => number = Math.random): number {
  const base = Math.min(30_000, 1000 * 2 ** Math.max(0, attempt))
  return Math.round(base * (0.8 + random() * 0.4))
}

export interface JellyfinSocketOptions {
  createSocket?: (url: string) => WebSocketLike
  random?: () => number
  /** Keep-alive period before the server says otherwise (ms). */
  defaultKeepAliveMs?: number
}

type Listener<T> = (value: T) => void

export class JellyfinSocket {
  private ws: WebSocketLike | null = null
  private statusValue: SocketStatus = 'idle'
  private attempt = 0
  private wanted = false
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private keepAliveTimer: ReturnType<typeof setInterval> | null = null
  private keepAliveMs: number
  private lastMessageAt = 0
  private readonly messageListeners = new Set<Listener<SocketMessage>>()
  private readonly statusListeners = new Set<Listener<SocketStatus>>()
  private readonly createSocket: (url: string) => WebSocketLike
  private readonly random: () => number
  private readonly onOnline = () => {
    if (this.wanted && this.statusValue === 'reconnecting') this.reconnectNow()
  }

  constructor(
    private readonly url: string,
    opts: JellyfinSocketOptions = {},
  ) {
    this.createSocket = opts.createSocket ?? ((u) => new WebSocket(u) as unknown as WebSocketLike)
    this.random = opts.random ?? Math.random
    this.keepAliveMs = opts.defaultKeepAliveMs ?? 30_000
  }

  get status(): SocketStatus {
    return this.statusValue
  }

  connect(): void {
    if (this.wanted) return
    this.wanted = true
    if (typeof window !== 'undefined') window.addEventListener('online', this.onOnline)
    this.open('connecting')
  }

  /** Closes for good (no reconnection). */
  close(): void {
    this.wanted = false
    if (typeof window !== 'undefined') window.removeEventListener('online', this.onOnline)
    this.clearTimers()
    const ws = this.ws
    this.ws = null
    if (ws) {
      ws.onopen = ws.onclose = ws.onerror = ws.onmessage = null
      try {
        ws.close(1000)
      } catch {
        /* already closed */
      }
    }
    this.setStatus('closed')
  }

  send(type: string, data?: unknown): boolean {
    if (!this.ws || this.ws.readyState !== OPEN) return false
    this.ws.send(
      JSON.stringify(
        data === undefined ? { MessageType: type } : { MessageType: type, Data: data },
      ),
    )
    return true
  }

  onMessage(listener: Listener<SocketMessage>): () => void {
    this.messageListeners.add(listener)
    return () => this.messageListeners.delete(listener)
  }

  onStatus(listener: Listener<SocketStatus>): () => void {
    this.statusListeners.add(listener)
    return () => this.statusListeners.delete(listener)
  }

  /** Resolves once the socket is open (rejects after `timeoutMs`). */
  whenOpen(timeoutMs = 10_000): Promise<void> {
    if (this.statusValue === 'open') return Promise.resolve()
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        off()
        reject(new Error('Socket did not open'))
      }, timeoutMs)
      const off = this.onStatus((s) => {
        if (s === 'open') {
          clearTimeout(timer)
          off()
          resolve()
        } else if (s === 'closed') {
          clearTimeout(timer)
          off()
          reject(new Error('Socket closed'))
        }
      })
    })
  }

  // ------------------------------------------------------------------ internals

  private setStatus(status: SocketStatus): void {
    if (status === this.statusValue) return
    this.statusValue = status
    for (const l of [...this.statusListeners]) l(status)
  }

  private open(status: SocketStatus): void {
    this.setStatus(status)
    let ws: WebSocketLike
    try {
      ws = this.createSocket(this.url)
    } catch {
      this.scheduleReconnect()
      return
    }
    this.ws = ws
    ws.onopen = () => {
      if (this.ws !== ws) return
      this.attempt = 0
      this.lastMessageAt = Date.now()
      this.startKeepAlive()
      this.setStatus('open')
    }
    ws.onmessage = (ev) => {
      if (this.ws !== ws) return
      this.lastMessageAt = Date.now()
      let msg: SocketMessage
      try {
        msg = JSON.parse(String(ev.data)) as SocketMessage
      } catch {
        return
      }
      if (!msg || typeof msg.MessageType !== 'string') return
      if (msg.MessageType === 'ForceKeepAlive') {
        const seconds = Number(msg.Data)
        // Jellyfin's own clients send a keep-alive at half the timeout it announces.
        if (seconds > 0) this.keepAliveMs = (seconds * 1000) / 2
        this.startKeepAlive()
        this.send('KeepAlive')
      }
      for (const l of [...this.messageListeners]) l(msg)
    }
    ws.onerror = () => {
      /* onclose follows */
    }
    ws.onclose = () => {
      if (this.ws !== ws) return
      this.ws = null
      this.stopKeepAlive()
      if (this.wanted) this.scheduleReconnect()
      else this.setStatus('closed')
    }
  }

  private startKeepAlive(): void {
    this.stopKeepAlive()
    this.keepAliveTimer = setInterval(() => {
      // Nothing heard for 2.5 keep-alive periods (the server answers each one): the link is dead.
      if (Date.now() - this.lastMessageAt > this.keepAliveMs * 2.5) {
        this.dropAndReconnect()
        return
      }
      this.send('KeepAlive')
    }, this.keepAliveMs)
  }

  private stopKeepAlive(): void {
    if (this.keepAliveTimer) clearInterval(this.keepAliveTimer)
    this.keepAliveTimer = null
  }

  private dropAndReconnect(): void {
    const ws = this.ws
    this.ws = null
    this.stopKeepAlive()
    if (ws) {
      ws.onopen = ws.onclose = ws.onerror = ws.onmessage = null
      try {
        ws.close()
      } catch {
        /* ignore */
      }
    }
    if (this.wanted) this.scheduleReconnect()
  }

  private scheduleReconnect(): void {
    this.setStatus('reconnecting')
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer)
    const delay = backoffDelay(this.attempt++, this.random)
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null
      if (this.wanted) this.open('reconnecting')
    }, delay)
  }

  private reconnectNow(): void {
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer)
    this.reconnectTimer = null
    this.attempt = 0
    this.open('reconnecting')
  }

  private clearTimers(): void {
    this.stopKeepAlive()
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer)
    this.reconnectTimer = null
  }
}
