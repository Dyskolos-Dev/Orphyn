// Global CSS + design tokens (owner: design agent). Imported once by main.tsx.
import '@fontsource-variable/inter'
import './tokens.css'
import './base.css'
import './typography.css'
import './materials.css'
import './interface-styles.css'
import { applyStoredTheme } from '@renderer/design/theme/theme'
import { applyStoredInterfaceStyle } from '@renderer/design/theme/interfaceStyle'

// Apply the persisted theme and interface style before React renders, so there is no flash on
// launch.
applyStoredTheme()
applyStoredInterfaceStyle()

export {}
