import { randomUUID } from 'node:crypto'
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs'
import { dirname } from 'node:path'
import type { AccountsStatus, NewServerAccount, ServerAccount } from '@shared/jellyfin'

/**
 * The subset of Electron's safeStorage the store needs (injectable for tests). Only the async
 * API: the synchronous one is deprecated since Electron 45 and logs a warning on every call.
 */
export interface SecureStorage {
  isAsyncEncryptionAvailable(): Promise<boolean>
  encryptStringAsync(plain: string): Promise<Buffer>
  decryptStringAsync(encrypted: Buffer): Promise<{ result: string; shouldReEncrypt: boolean }>
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
  'Secure storage is unavailable on this system: access tokens are kept in memory only and you will have to sign in again after restarting Lumen.'

export class AccountStore {
  private readonly deps: Required<AccountStoreDeps>
  private data: StoredFile
  /** Clear tokens, in memory only. Authoritative source for getToken. */
  private readonly tokens = new Map<string, string>()
  /** base64 ciphertext of each token: what `save` writes. */
  private readonly ciphers = new Map<string, string>()

  /** Use `AccountStore.open`: encryption availability and decryption are asynchronous. */
  private constructor(
    deps: AccountStoreDeps,
    private readonly encryptionAvailable: boolean,
  ) {
    this.deps = { now: () => new Date(), uuid: randomUUID, ...deps }
    this.data = this.emptyFile()
  }

  static async open(deps: AccountStoreDeps): Promise<AccountStore> {
    const s = deps.storage
    const available =
      (await s.isAsyncEncryptionAvailable()) && s.getSelectedStorageBackend?.() !== 'basic_text'
    const store = new AccountStore(deps, available)
    await store.load()
    return store
  }

  get encrypted(): boolean {
    return this.encryptionAvailable
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

  /**
   * Adds an account; the same server+user is updated in place (new token, new url/name). Two
   * entries for one user could not both work anyway: Jellyfin revokes the previous token of a
   * device when it signs in again.
   */
  async add(input: NewServerAccount): Promise<ServerAccount> {
    const cipher = this.encrypted
      ? (await this.deps.storage.encryptStringAsync(input.accessToken)).toString('base64')
      : null
    const now = this.deps.now().toISOString()
    const existing = this.data.accounts.find(
      (a) => a.serverId === input.serverId && a.userId === input.userId,
    )
    const stored: StoredAccount = {
      id: existing?.id ?? this.deps.uuid(),
      // Signing in again without a name keeps the one the user chose.
      name: input.name.trim() || existing?.name || input.serverName,
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
    if (cipher) this.ciphers.set(stored.id, cipher)
    else this.ciphers.delete(stored.id)
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

  /**
   * Forgets an account. When it was the active one, the most recently used remaining account
   * takes over (only removing the last one leads back to onboarding).
   */
  remove(id: string): void {
    this.data.accounts = this.data.accounts.filter((a) => a.id !== id)
    this.tokens.delete(id)
    this.ciphers.delete(id)
    if (this.data.activeId === id) {
      const next = [...this.data.accounts].sort((a, b) =>
        b.lastUsedAt.localeCompare(a.lastUsedAt),
      )[0]
      this.data.activeId = next?.id ?? null
    }
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

  private emptyFile(): StoredFile {
    return { version: 1, deviceId: this.deps.uuid(), activeId: null, accounts: [] }
  }

  private async load(): Promise<void> {
    if (!existsSync(this.deps.filePath)) {
      this.save()
      return
    }
    let parsed: Partial<StoredFile>
    try {
      parsed = JSON.parse(readFileSync(this.deps.filePath, 'utf8')) as Partial<StoredFile>
    } catch {
      this.save() // corrupt file: start clean (user signs in again)
      return
    }
    this.data = {
      version: 1,
      deviceId: parsed.deviceId || this.data.deviceId,
      activeId: parsed.activeId ?? null,
      accounts: Array.isArray(parsed.accounts) ? parsed.accounts : [],
    }
    let reEncrypted = false
    for (const a of this.data.accounts) {
      if (!a.accessToken || !this.encrypted) continue
      try {
        const { result, shouldReEncrypt } = await this.deps.storage.decryptStringAsync(
          Buffer.from(a.accessToken, 'base64'),
        )
        this.tokens.set(a.id, result)
        let cipher = a.accessToken
        if (shouldReEncrypt) {
          cipher = (await this.deps.storage.encryptStringAsync(result)).toString('base64')
          reEncrypted = true
        }
        this.ciphers.set(a.id, cipher)
      } catch {
        // undecryptable (keychain changed): token lost, user must log in again
      }
    }
    if (reEncrypted) this.save()
  }

  private save(): void {
    const out: StoredFile = {
      ...this.data,
      accounts: this.data.accounts.map((a) => ({
        ...a,
        // Never write a clear-text token: without encryption nothing is persisted.
        accessToken: this.encrypted ? (this.ciphers.get(a.id) ?? null) : null,
      })),
    }
    mkdirSync(dirname(this.deps.filePath), { recursive: true })
    const tmp = `${this.deps.filePath}.tmp`
    writeFileSync(tmp, JSON.stringify(out, null, 2), 'utf8')
    renameSync(tmp, this.deps.filePath)
  }
}
