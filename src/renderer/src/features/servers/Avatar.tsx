import { useState } from 'react'
import { initials } from '@renderer/design'
import type { ServerAccount } from '@shared/jellyfin'

/** Jellyfin profile picture of the account's user, or their initials. */
export function UserAvatar({
  account,
  size = 28,
}: {
  account: Pick<ServerAccount, 'url' | 'userId' | 'userName'>
  size?: number
}) {
  const src = `${account.url}/Users/${account.userId}/Images/Primary?maxWidth=${size * 2}`
  const [failed, setFailed] = useState<string | null>(null)
  return (
    <span className="jr-avatar" style={{ width: size, height: size }} aria-hidden="true">
      {failed === src ? (
        initials(account.userName)
      ) : (
        <img src={src} alt="" draggable={false} onError={() => setFailed(src)} />
      )}
    </span>
  )
}
