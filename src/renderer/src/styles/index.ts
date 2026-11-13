// Global CSS + design tokens (owner: design agent). Imported once by main.tsx.
import '@fontsource-variable/inter'
import './tokens.css'
import './base.css'
import './typography.css'
import './materials.css'
import { applyStoredTheme } from '@renderer/design/theme/theme'

// Apply the persisted theme before React renders, so there is no light/dark flash on launch.
applyStoredTheme()

export {}
