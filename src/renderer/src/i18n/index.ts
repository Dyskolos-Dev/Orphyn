import i18n from 'i18next'
import LanguageDetector from 'i18next-browser-languagedetector'
import { initReactI18next } from 'react-i18next'

// Every file at ./locales/<lng>/<namespace>.json is loaded automatically.
const modules = import.meta.glob<Record<string, unknown>>('./locales/*/*.json', {
  eager: true,
  import: 'default',
})

const resources: Record<string, Record<string, Record<string, unknown>>> = {}
const namespaces = new Set<string>()
for (const [path, content] of Object.entries(modules)) {
  const match = /\.\/locales\/([^/]+)\/([^/]+)\.json$/.exec(path)
  const lng = match?.[1]
  const ns = match?.[2]
  if (!lng || !ns) continue
  namespaces.add(ns)
  const bundle = (resources[lng] ??= {})
  bundle[ns] = content
}

export const SUPPORTED_LANGUAGES = ['fr', 'en'] as const

void i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    ns: [...namespaces],
    defaultNS: 'common',
    fallbackLng: 'en',
    supportedLngs: [...SUPPORTED_LANGUAGES],
    nonExplicitSupportedLngs: true,
    load: 'languageOnly',
    // Stored user choice first, then the OS language (navigator.language).
    detection: { order: ['localStorage', 'navigator'], caches: ['localStorage'] },
    interpolation: { escapeValue: false },
  })

export default i18n
