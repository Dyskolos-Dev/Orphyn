import type { RouteObject } from 'react-router'
import { OnboardingPage } from './OnboardingPage'
import './register'

/** Route table for the servers feature. Owned by the servers agent. */
export const serversRoutes: RouteObject[] = [{ path: '/onboarding', element: <OnboardingPage /> }]
