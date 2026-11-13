/** Parses "1.2.3" (a leading "v" and any "-pre" / "+build" suffix are ignored). */
export function parseVersion(v: string): [number, number, number] | null {
  const m = /^v?(\d+)\.(\d+)\.(\d+)/.exec(v.trim())
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null
}

/** Negative when a < b, 0 when equal, positive when a > b. Unparsable versions sort first. */
export function compareVersions(a: string, b: string): number {
  const pa = parseVersion(a)
  const pb = parseVersion(b)
  if (!pa || !pb) return pa ? 1 : pb ? -1 : 0
  for (let i = 0; i < 3; i++) {
    const d = pa[i]! - pb[i]!
    if (d !== 0) return d
  }
  return 0
}
