/**
 * Minimal Jellyfin client for the suite itself: picks test items and checks server-side state
 * (resume position, played state). Signs in under its own device id so it never revokes the
 * app's session.
 */
import type { Credentials } from './app'

const AUTH =
  'MediaBrowser Client="Orphyn E2E", Device="e2e", DeviceId="orphyn-e2e-probe", Version="0.1.0"'

export interface Item {
  Id: string
  Name: string
  Type: string
  RunTimeTicks?: number
  ProductionYear?: number
  AlbumArtist?: string
  ChildCount?: number
  SeriesName?: string
  UserData?: { PlaybackPositionTicks?: number; Played?: boolean; LastPlayedDate?: string }
  MediaStreams?: {
    Type: string
    Index: number
    Language?: string
    IsTextSubtitleStream?: boolean
  }[]
}

export class JellyfinProbe {
  private token = ''
  userId = ''

  constructor(private readonly creds: Credentials) {}

  private headers(): Record<string, string> {
    return {
      'Content-Type': 'application/json',
      Authorization: this.token ? `${AUTH}, Token="${this.token}"` : AUTH,
    }
  }

  async login(): Promise<void> {
    const res = await fetch(`${this.creds.url}/Users/AuthenticateByName`, {
      method: 'POST',
      headers: this.headers(),
      body: JSON.stringify({ Username: this.creds.user, Pw: this.creds.password }),
    })
    if (!res.ok) throw new Error(`E2E probe login failed: HTTP ${res.status}`)
    const body = (await res.json()) as { AccessToken: string; User: { Id: string } }
    this.token = body.AccessToken
    this.userId = body.User.Id
  }

  async logout(): Promise<void> {
    if (!this.token) return
    await fetch(`${this.creds.url}/Sessions/Logout`, { method: 'POST', headers: this.headers() })
    this.token = ''
  }

  async get<T>(path: string, params: Record<string, string | number | boolean> = {}): Promise<T> {
    const q = new URLSearchParams(Object.entries(params).map(([k, v]) => [k, String(v)]))
    const res = await fetch(`${this.creds.url}${path}${q.size ? `?${q}` : ''}`, {
      headers: this.headers(),
    })
    if (!res.ok) throw new Error(`E2E probe GET ${path} failed: HTTP ${res.status}`)
    return (await res.json()) as T
  }

  async status(path: string): Promise<number> {
    const res = await fetch(`${this.creds.url}${path}`, { headers: this.headers() })
    return res.status
  }

  async items(params: Record<string, string | number | boolean>): Promise<Item[]> {
    const r = await this.get<{ Items: Item[] }>('/Items', {
      userId: this.userId,
      recursive: true,
      ...params,
    })
    return r.Items
  }

  item(id: string): Promise<Item> {
    return this.get<Item>(`/Items/${id}`, { userId: this.userId, fields: 'MediaStreams' })
  }

  /** Back to unwatched with no resume point (marking it played clears the position first). */
  async resetProgress(id: string): Promise<void> {
    for (const method of ['POST', 'DELETE']) {
      await fetch(`${this.creds.url}/UserPlayedItems/${id}?userId=${this.userId}`, {
        method,
        headers: this.headers(),
      })
    }
  }
}
