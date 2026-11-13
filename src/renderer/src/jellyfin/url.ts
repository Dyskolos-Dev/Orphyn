/** Server URL normalisation for the "add server" flow. */

const DEFAULT_PORT = '8096'

/**
 * Turns what a user typed ("nas", "192.168.1.5:8096", "https://media.example.com/jellyfin/") into
 * an ordered list of base URLs worth probing (no trailing slash). Returns [] for unusable input.
 *
 * - Missing scheme: `http://host:8096` first for bare hosts (typical LAN install), then https, then http.
 * - http without a port also tries :8096.
 * - A bare host with no path also tries the `/jellyfin` base path last.
 * - A pasted `/web/...` UI address is cut back to the server root.
 */
export function urlCandidates(input: string): string[] {
  const raw = input.trim()
  if (!raw) return []
  const hasScheme = /^[a-z][a-z0-9+.-]*:\/\//i.test(raw)
  if (hasScheme && !/^https?:\/\//i.test(raw)) return []
  let parsed: URL
  try {
    parsed = new URL(hasScheme ? raw : `http://${raw}`)
  } catch {
    return []
  }
  if (!parsed.hostname) return []

  let path = parsed.pathname.replace(/\/web(\/.*)?$/i, '').replace(/\/+$/, '')
  if (path === '/') path = ''
  const host = parsed.hostname // IPv6 literals already carry their brackets
  const hasPort = parsed.port !== ''
  const explicitPort = hasPort ? `:${parsed.port}` : ''

  const bases: string[] = []
  const add = (scheme: string, port: string): void => {
    const base = `${scheme}://${host}${port}`
    if (!bases.includes(base)) bases.push(base)
  }

  if (hasScheme) {
    const scheme = parsed.protocol.replace(':', '')
    add(scheme, explicitPort)
    if (scheme === 'http' && !hasPort) add('http', `:${DEFAULT_PORT}`)
  } else if (hasPort) {
    add('http', explicitPort)
    add('https', explicitPort)
  } else {
    add('http', `:${DEFAULT_PORT}`)
    add('https', '')
    add('http', '')
  }

  const out: string[] = []
  for (const base of bases) out.push(`${base}${path}`)
  if (!path) for (const base of bases) out.push(`${base}/jellyfin`)
  return out
}

/** The primary (first) candidate, or null when the input is not a usable URL. */
export function normalizeServerUrl(input: string): string | null {
  return urlCandidates(input)[0] ?? null
}
