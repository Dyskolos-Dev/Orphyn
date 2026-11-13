import { Clapperboard } from 'lucide-react'
import { useNavigate } from 'react-router'
import { ShellImmersive } from '@renderer/shell'
import { ConnectFlow } from './ConnectFlow'
import './connect.css'

/** First-launch screen: a calm, centred, full-window server setup. */
export function OnboardingPage() {
  const navigate = useNavigate()
  return (
    <div className="jr-onboarding">
      <ShellImmersive />
      <div>
        <Clapperboard className="jr-onboarding__logo" strokeWidth={1.25} aria-hidden="true" />
        <ConnectFlow onDone={() => void navigate('/video', { replace: true })} />
      </div>
    </div>
  )
}
