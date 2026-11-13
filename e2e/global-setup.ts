import { newProfileDir } from './support/app'

/** One fresh user data directory per `npm run e2e` run, shared by every scenario file. */
export default function globalSetup(): void {
  process.env['JELLYRA_E2E_PROFILE'] ||= newProfileDir()
  console.log(`[e2e] user data directory: ${process.env['JELLYRA_E2E_PROFILE']}`)
}
