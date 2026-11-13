import { useEffect, useState, type ReactNode } from 'react'
import { useSession } from '@renderer/jellyfin'
import { version } from '../../../../../package.json'
import { bundledNotes, notesToShow, type VersionNotes } from './notes'
import { readLastSeen, writeLastSeen } from './storage'
import { WhatsNewSheet } from './WhatsNewSheet'
import './whatsnew.css'

/**
 * Shell provider: once the session is known, compares the app version with
 * `lumen.lastSeenVersion`, shows the notes of every newer version once, then stores the version.
 * A fresh install (no stored value, no account) only stores it: onboarding already greets.
 */
export function WhatsNewHost({ children }: { children: ReactNode }) {
  const status = useSession((s) => s.status)
  const [notes, setNotes] = useState<VersionNotes[]>([])

  useEffect(() => {
    if (status === 'idle' || status === 'loading') return
    const toShow = notesToShow({
      lastSeen: readLastSeen(),
      current: version,
      hasAccount: status !== 'no-server',
      notes: bundledNotes(),
    })
    if (toShow.length > 0) setNotes(toShow)
    else writeLastSeen(version)
  }, [status])

  return (
    <>
      {children}
      <WhatsNewSheet
        open={notes.length > 0}
        notes={notes}
        onClose={() => {
          writeLastSeen(version)
          setNotes([])
        }}
      />
    </>
  )
}
