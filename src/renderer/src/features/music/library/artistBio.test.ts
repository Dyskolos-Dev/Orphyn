import { describe, expect, it } from 'vitest'
import { acceptWikipediaSummary, cleanOverview, wikipediaSummaryUrl } from './artistBio'

describe('acceptWikipediaSummary', () => {
  it('accepts a musical act', () => {
    expect(
      acceptWikipediaSummary({
        type: 'standard',
        description: 'English rock band',
        extract: 'Muse are an English rock band.',
      }),
    ).toBe('Muse are an English rock band.')
    expect(
      acceptWikipediaSummary({ description: 'groupe de rock français', extract: 'Texte.' }),
    ).toBe('Texte.')
    expect(acceptWikipediaSummary({ description: 'Rappeur français', extract: 'Texte.' })).toBe(
      'Texte.',
    )
  })
  it('rejects look-alikes, disambiguations and empty pages', () => {
    expect(
      acceptWikipediaSummary({ description: 'Greek mythological figure', extract: 'Muses…' }),
    ).toBeNull()
    expect(
      acceptWikipediaSummary({
        type: 'disambiguation',
        description: 'Topics referred to by the same term (band, singer)',
        extract: 'x',
      }),
    ).toBeNull()
    expect(acceptWikipediaSummary({ description: 'singer', extract: '  ' })).toBeNull()
    expect(acceptWikipediaSummary({ extract: 'No description' })).toBeNull()
    expect(acceptWikipediaSummary(null)).toBeNull()
  })
})

describe('helpers', () => {
  it('builds an encoded summary URL', () => {
    expect(wikipediaSummaryUrl('AC/DC', 'fr')).toBe(
      'https://fr.wikipedia.org/api/rest_v1/page/summary/AC%2FDC?redirect=true',
    )
    expect(wikipediaSummaryUrl(' Daft  Punk ', 'en')).toContain('/summary/Daft_Punk?')
  })
  it('strips markup from an overview', () => {
    expect(cleanOverview('<p>Hello<br/>world</p>')).toBe('Hello\nworld')
    expect(cleanOverview(null)).toBe('')
  })
})
