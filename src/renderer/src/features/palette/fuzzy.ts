/**
 * Fuzzy matching for the command palette: accent- and case-insensitive subsequence match that
 * rewards consecutive letters, word starts and early matches, and returns the matched positions
 * (indices into the ORIGINAL text) so the UI can highlight them.
 */

export interface FuzzyMatch {
  score: number
  /** Indices of the matched characters in the original text, ascending. */
  indices: number[]
}

const COMBINING = /[̀-ͯ]/g
const SEPARATORS = /[\s\-_.:/,–—·]/
const MAX_TEXT = 120

/** Lower-case without accents ("Éléphant" -> "elephant"). */
export function normalize(s: string): string {
  return s.normalize('NFD').replace(COMBINING, '').toLowerCase()
}

/** One normalized char per kept original index, so highlights map back onto the original text. */
function fold(text: string): { chars: string[]; orig: number[] } {
  const chars: string[] = []
  const orig: number[] = []
  const limit = Math.min(text.length, MAX_TEXT)
  for (let i = 0; i < limit; i++) {
    const n = (text[i] as string).normalize('NFD').replace(COMBINING, '').toLowerCase()
    if (!n) continue
    chars.push(n[0] as string)
    orig.push(i)
  }
  return { chars, orig }
}

const MATCH = 10
const START_BONUS = 12
const WORD_BONUS = 8
const CONSECUTIVE_BONUS = 9
const GAP_PENALTY = 1.5
const LEADING_PENALTY = 0.5

/** Best fuzzy alignment of `query` in `text`, or null when some query letter is missing. */
export function fuzzyMatch(query: string, text: string): FuzzyMatch | null {
  const q = Array.from(normalize(query)).filter((c) => !/\s/.test(c))
  if (q.length === 0) return null
  const { chars: t, orig } = fold(text)
  const n = t.length
  const m = q.length
  if (m > n) return null

  const base = (i: number): number => {
    let s = MATCH
    if (i === 0) s += START_BONUS
    else if (SEPARATORS.test(t[i - 1] as string)) s += WORD_BONUS
    return s
  }

  // score[j][i]: best score with q[j] matched at t[i]; from[j][i]: where q[j-1] was matched.
  const NEG = -Infinity
  const score: number[][] = Array.from({ length: m }, () => new Array<number>(n).fill(NEG))
  const from: number[][] = Array.from({ length: m }, () => new Array<number>(n).fill(-1))

  for (let i = 0; i < n; i++) {
    if (t[i] === q[0]) score[0]![i] = base(i) - i * LEADING_PENALTY
  }
  for (let j = 1; j < m; j++) {
    for (let i = j; i < n; i++) {
      if (t[i] !== q[j]) continue
      let best = NEG
      let bestK = -1
      for (let k = j - 1; k < i; k++) {
        const prev = score[j - 1]![k] as number
        if (prev === NEG) continue
        const s = prev + (k === i - 1 ? CONSECUTIVE_BONUS : -(i - k - 1) * GAP_PENALTY)
        if (s > best) {
          best = s
          bestK = k
        }
      }
      if (bestK >= 0) {
        score[j]![i] = best + base(i)
        from[j]![i] = bestK
      }
    }
  }

  let end = -1
  let top = NEG
  for (let i = m - 1; i < n; i++) {
    const s = score[m - 1]![i] as number
    if (s > top) {
      top = s
      end = i
    }
  }
  if (end < 0) return null

  const indices = new Array<number>(m)
  let i = end
  for (let j = m - 1; j >= 0; j--) {
    indices[j] = orig[i] as number
    i = from[j]![i] as number
  }

  // A contiguous hit of the whole query is always the better answer: reward it explicitly.
  const flat = t.join('')
  const needle = q.join('')
  const at = flat.indexOf(needle)
  let final = top
  if (at >= 0) {
    final += 40 + (at === 0 ? 30 : SEPARATORS.test(flat[at - 1] ?? ' ') ? 15 : 0)
    if (flat === needle) final += 60
  }
  return { score: final, indices }
}

export interface Ranked<T> {
  item: T
  score: number
  /** Highlight positions in the item's primary text (empty when an alias matched). */
  indices: number[]
  /** Position in the input list (stable ordering for equal scores). */
  order: number
}

/** Below this a match is noise (a scattering of letters over a long text). */
const MIN_SCORE = 18
const ALIAS_PENALTY = 6

/**
 * Ranks `items` against `query`. `texts(item)` returns the primary text (highlighted) followed by
 * aliases (other-language name, synonyms) that also match but do not highlight.
 */
export function rank<T>(
  items: readonly T[],
  query: string,
  texts: (item: T) => readonly string[],
): Ranked<T>[] {
  const out: Ranked<T>[] = []
  items.forEach((item, order) => {
    let best: Ranked<T> | null = null
    texts(item).forEach((text, k) => {
      if (!text) return
      const m = fuzzyMatch(query, text)
      if (!m) return
      const score = m.score - (k === 0 ? 0 : ALIAS_PENALTY)
      if (!best || score > best.score) {
        best = { item, score, indices: k === 0 ? m.indices : [], order }
      }
    })
    const found = best as Ranked<T> | null
    if (found && found.score >= MIN_SCORE) out.push(found)
  })
  return out.sort((a, b) => b.score - a.score || a.order - b.order)
}

/** Splits `text` into runs, flagging the ones covered by `indices` (for <mark> rendering). */
export function highlightRuns(
  text: string,
  indices: readonly number[],
): { text: string; match: boolean }[] {
  if (indices.length === 0) return [{ text, match: false }]
  const set = new Set(indices)
  const runs: { text: string; match: boolean }[] = []
  for (let i = 0; i < text.length; i++) {
    const match = set.has(i)
    const last = runs[runs.length - 1]
    if (last && last.match === match) last.text += text[i]
    else runs.push({ text: text[i] as string, match })
  }
  return runs
}
