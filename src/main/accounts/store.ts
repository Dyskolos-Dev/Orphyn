import { randomUUID } from 'node:crypto'
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs'
import { dirname } from 'node:path'
import type { AccountsStatus, NewServerAccount, ServerAccount } from '@shared/jellyfin'

/** The subset of Electron's safeStorage the store needs (injectable for tests). */
export interface SecureStorage {
  isEncryptionAvailable(): boolean
  encryptString(plain: string): Buffer
  decryptString(encrypted: Buffer): string
  /** Linux only: 'basic_text' means no real keyring, i.e. effectively clear text. */
  getSelectedStorageBackend?(): string
}

export interface AccountStoreDeps {
  filePath: string
  storage: SecureStorage
  now?: () => Date
  uuid?: () => string
}

/** On-disk record: the token is base64 of the safeStorage ciphertext, never clear text. */
interface StoredAccount extends Omit<ServerAccount, 'hasToken'> {
  accessToken: string | null
}

interface StoredFile {
  version: 1
  deviceId: string
  activeId: string | null
  accounts: StoredAccount[]
}

const NO_ENCRYPTION_WARNING =
  'Secure storage is unavailable on this system: access tokens are kept in memory only and you will have to sign in again after restarting Jellyra.'

export class AccountStore {
  private readonly deps: Required<AccountStoreDeps>
  private data: StoredFile
  /** Clear tokens, in memory only. Authoritative source for getToken. */
  private readonly tokens = new Map<string, string>()

  constructor(deps: AccountStoreDeps) {
    this.deps = { now: () => new Date(), uuid: randomUUID, ...deps }
    this.data = this.load()
  }

  get encrypted(): boolean {
    const s = this.deps.storage
    if (!s.isEncryptionAvailable()) return false
    return s.getSelectedStorageBackend?.() !== 'basic_text'
  }

  getStatus(): AccountsStatus {
    return this.encrypted
      ? { encrypted: true }
      : { encrypted: false, warning: NO_ENCRYPTION_WARNING }
  }

  getDeviceId(): string {
    return this.data.deviceId
  }

  list(): ServerAccount[] {
    return this.data.accounts.map((a) => this.toPublic(a))
  }

  getActive(): ServerAccount | null {
    const a = this.data.accounts.find((x) => x.id === this.data.activeId)
    return a ? this.toPublic(a) : null
  }

  getToken(id: string): string | null {
    return this.tokens.get(id) ?? null
  }

  /** Adds an account; the same server+user is updated in place (new token, new url/name). */
  add(input: NewServerAccount): ServerAccount {
    const now = this.deps.now().toISOString()
    const existing = this.data.accounts.find(
      (a) => a.serverId === input.serverId && a.userId === input.userId,
    )
    const stored: StoredAccount = {
      id: existing?.id ?? this.deps.uuid(),
      name: input.name.trim() || input.serverName,
      url: input.url,
      serverId: input.serverId,
      serverName: input.serverName,
      version: input.version,
      userId: input.userId,
      userName: input.userName,
      accessToken: null,
      addedAt: existing?.addedAt ?? now,
      lastUsedAt: now,
    }
    if (existing) Object.assign(existing, stored)
    else this.data.accounts.push(stored)
    this.tokens.set(stored.id, input.accessToken)
    if (!this.data.activeId) this.data.activeId = stored.id
    this.save()
    return this.toPublic(existing ?? stored)
  }

  rename(id: string, name: string): ServerAccount | null {
    const a = this.data.accounts.find((x) => x.id === id)
    if (!a) return null
    a.name = name.trim() || a.name
    this.save()
    return this.toPublic(a)
  }

  remove(id: string): void {
    this.data.accounts = this.data.accounts.filter((a) => a.id !== id)
    this.tokens.delete(id)
    if (this.data.activeId === id) this.data.activeId = null
    this.save()
  }

  setActive(id: string | null): ServerAccount | null {
    if (id === null) {
      this.data.activeId = null
      this.save()
      return null
    }
    const a = this.data.accounts.find((x) => x.id === id)
    if (!a) return null
    this.data.activeId = id
    a.lastUsedAt = this.deps.now().toISOString()
    this.save()
    return this.toPublic(a)
  }

  private toPublic(a: StoredAccount): ServerAccount {
    return {
      id: a.id,
      name: a.name,
      url: a.url,
      serverId: a.serverId,
      serverName: a.serverName,
      version: a.version,
      userId: a.userId,
      userName: a.userName,
      addedAt: a.addedAt,
      lastUsedAt: a.lastUsedAt,
      hasToken: this.tokens.has(a.id),
    }
  }

  private load(): StoredFile {
    const fresh: StoredFile = {
      version: 1,
      deviceId: this.deps.uuid(),
      activeId: null,
      accounts: [],
    }
    if (!existsSync(this.deps.filePath)) {
      this.data = fresh
      this.save()
      return fresh
    }
    let parsed: Partial<StoredFile>
    try {
      parsed = JSON.parse(readFileSync(this.deps.filePath, 'utf8')) as Partial<StoredFile>
    } catch {
      this.data = fresh // corrupt file: start clean (user signs in again)
      this.save()
      return fresh
    }
    const data: StoredFile = {
      version: 1,
      deviceId: parsed.deviceId || fresh.deviceId,
      activeId: parsed.activeId ?? null,
      accounts: Array.isArray(parsed.accounts) ? parsed.accounts : [],
    }
    for (const a of data.accounts) {
      if (!a.accessToken || !this.encrypted) continue
      try {
        this.tokens.set(a.id, this.deps.storage.decryptString(Buffer.from(a.accessToken, 'base64')))
      } catch {
        // undecryptable (keychain changed): token lost, user must log in again
      }
    }
    return data
  }

  private save(): void {
    const out: StoredFile = {
      ...this.data,
      accounts: this.data.accounts.map((a) => {
        const token = this.tokens.get(a.id)
        return {
          ...a,
          // Never write a clear-text token: without encryption nothing is persisted.
          accessToken:
            token && this.encrypted
              ? this.deps.storage.encryptString(token).toString('base64')
              : null,
        }
      }),
    }
    mkdirSync(dirname(this.deps.filePath), { recursive: true })
    const tmp = `${this.deps.filePath}.tmp`
    writeFileSync(tmp, JSON.stringify(out, null, 2), 'utf8')
    renameSync(tmp, this.deps.filePath)
  }
}
