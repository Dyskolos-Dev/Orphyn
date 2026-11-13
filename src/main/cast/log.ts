/** Logging helpers: every line is prefixed with [cast]. */
export const log = {
  info: (...args: unknown[]): void => console.log('[cast]', ...args),
  warn: (...args: unknown[]): void => console.warn('[cast]', ...args),
  error: (...args: unknown[]): void => console.error('[cast]', ...args),
}

export function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err)
}
