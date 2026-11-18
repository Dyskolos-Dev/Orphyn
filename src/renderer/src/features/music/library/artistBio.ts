export type BioSource = 'jellyfin' | 'wikipedia' | 'audiodb'

export interface Bio {
  text: string
  source: BioSource
}

/** Words that mean "this page is about a musical act" (Wikipedia short description). */
const MUSICAL =
  /\b(?:music(?:ian|al)?|musicien(?:ne)?s?|singer|chanteu(?:r|se)s?|band|groupe|rapper|rappeu(?:r|se)s?|duo|trio|songwriter|compositeu?r|composer|vocalist|dj|disc jockey|producer|producteur|artiste|rock|pop|hip[- ]hop|orchestra|orchestre|guitarist|guitariste|pianist|pianiste)\b/i

export interface WikipediaSummary {
  type?: string
  description?: string
  extract?: string
}

/**
 * A Wikipedia summary is only taken when it is not a disambiguation page and its short
 * description says it is about a musical act: "Muse" must not return the Greek goddess.
 */
export function acceptWikipediaSummary(s: WikipediaSummary | null | undefined): string | null {
  if (!s || s.type === 'disambiguation') return null
  const extract = s.extract?.trim()
  if (!extract) return null
  if (!s.description || !MUSICAL.test(s.description)) return null
  return extract
}

export function wikipediaSummaryUrl(name: string, lang: 'fr' | 'en'): string {
  return `https://${lang}.wikipedia.org/api/rest_v1/page/summary/${encodeURIComponent(
    name.trim().replace(/\s+/g, '_'),
  )}?redirect=true`
}

/** Jellyfin overviews sometimes hold HTML or link markup: show plain text. */
export function cleanOverview(raw: string | null | undefined): string {
  return (raw ?? '')
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<[^>]+>/g, '')
    .replace(/\[\/?[a-z]+(?:=[^\]]*)?\]/gi, '')
    .replace(/[ \t]+\n/g, '\n')
    .trim()
}
