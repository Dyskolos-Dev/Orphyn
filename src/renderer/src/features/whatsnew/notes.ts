import { compareVersions } from './version'

export interface ReleaseNoteItem {
  /** Key of `NOTE_ICONS` (SF Symbols-like glyphs). */
  icon: string
  title: string
  text: string
}
export interface ReleaseNoteText {
  title: string
  items: ReleaseNoteItem[]
}
export type ReleaseNote = Record<'fr' | 'en', ReleaseNoteText>
export interface VersionNotes {
  version: string
  note: ReleaseNote
}

/**
 * The first release that stored `lumen.lastSeenVersion` is 1.0.3: an install that has an account
 * but no stored value comes from this version or an earlier one.
 */
export const LEGACY_VERSION = '1.0.2'

const files = import.meta.glob<ReleaseNote>('./notes/*.json', {
  eager: true,
  import: 'default',
})

/** Every bundled release note, newest first. */
export function bundledNotes(): VersionNotes[] {
  return Object.entries(files)
    .map(([path, note]) => ({ version: /\/([^/]+)\.json$/.exec(path)![1]!, note }))
    .sort((a, b) => compareVersions(b.version, a.version))
}

export interface UpgradeInput {
  /** Stored `lumen.lastSeenVersion`, or null when never stored. */
  lastSeen: string | null
  /** Running app version. */
  current: string
  /** A server account already exists (the app was used before). */
  hasAccount: boolean
  notes: readonly VersionNotes[]
}

/**
 * Notes to present at launch, newest first. A fresh install (nothing stored, no account) shows
 * nothing; an unstored version with an account is an upgrade from `LEGACY_VERSION`; skipped
 * versions all show up. Versions newer than the running app are never shown.
 */
export function notesToShow({
  lastSeen,
  current,
  hasAccount,
  notes,
}: UpgradeInput): VersionNotes[] {
  const from = lastSeen ?? (hasAccount ? LEGACY_VERSION : null)
  if (from === null) return []
  return notes
    .filter((n) => compareVersions(n.version, from) > 0 && compareVersions(n.version, current) <= 0)
    .sort((a, b) => compareVersions(b.version, a.version))
}
