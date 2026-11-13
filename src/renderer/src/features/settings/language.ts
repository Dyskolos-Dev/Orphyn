/**
 * Interface language choice: `system` (follow the OS), `fr` or `en`. Persisted under
 * `lumen.language`; applied at module load (before the first render) and instantly on change.
 */
import i18n from '@renderer/i18n'

export type LanguageChoice = 'system' | 'fr' | 'en'
export const LANGUAGE_STORAGE_KEY = 'lumen.language'
export const LANGUAGE_CHOICES: readonly LanguageChoice[] = ['system', 'fr', 'en']

export function isLanguageChoice(v: unknown): v is LanguageChoice {
  return v === 'system' || v === 'fr' || v === 'en'
}

export function systemLanguage(): 'fr' | 'en' {
  const nav = typeof navigator !== 'undefined' ? navigator.language : 'en'
  return nav.toLowerCase().startsWith('fr') ? 'fr' : 'en'
}

export function getLanguageChoice(): LanguageChoice {
  try {
    const v = globalThis.localStorage?.getItem(LANGUAGE_STORAGE_KEY)
    return isLanguageChoice(v) ? v : 'system'
  } catch {
    return 'system'
  }
}

/** Stores the choice and switches the UI language right away. */
export async function setLanguageChoice(choice: LanguageChoice): Promise<void> {
  try {
    globalThis.localStorage?.setItem(LANGUAGE_STORAGE_KEY, choice)
  } catch {
    // Storage unavailable: still applies for this session.
  }
  await i18n.changeLanguage(choice === 'system' ? systemLanguage() : choice)
}

// Apply the stored choice at startup. Without one, the i18n detector (stored → OS) already did it.
const storedChoice = (() => {
  try {
    return globalThis.localStorage?.getItem(LANGUAGE_STORAGE_KEY)
  } catch {
    return null
  }
})()
if (isLanguageChoice(storedChoice)) {
  void i18n.changeLanguage(storedChoice === 'system' ? systemLanguage() : storedChoice)
}
