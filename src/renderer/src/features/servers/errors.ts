import type { TFunction } from 'i18next'
import type { ConnectResult, ProbeError } from '@renderer/jellyfin'

type AnyError = Extract<ConnectResult, { ok: false }>['error'] | ProbeError

/** Localised, user-facing text for a probe / login error from the data layer. */
export function errorMessage(t: TFunction<'servers'>, error: AnyError): string {
  switch (error.kind) {
    case 'version_too_old': {
      const version = 'version' in error ? error.version : undefined
      return version
        ? t('errors.version_too_old', { version })
        : t('errors.version_too_old_unknown')
    }
    case 'invalid_url':
    case 'unreachable':
    case 'not_jellyfin':
    case 'tls':
    case 'invalid_credentials':
    case 'server_error':
      return t(`errors.${error.kind}`)
    default:
      return t('errors.generic')
  }
}
