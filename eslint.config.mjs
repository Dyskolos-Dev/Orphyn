import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import tseslint from 'typescript-eslint'

export default tseslint.config(
  { ignores: ['node_modules/**', 'out/**', 'dist/**', 'release/**', 'coverage/**', 'src-tauri/target/**', 'src-tauri/gen/**', 'dist-renderer/**', 'site/**'] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ['src/renderer/**/*.{ts,tsx}'],
    languageOptions: { globals: globals.browser },
    plugins: { 'react-hooks': reactHooks },
    rules: {
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',
    },
  },
  {
    files: ['src/main/**/*.ts', 'src/preload/**/*.ts', '*.config.{ts,mjs}'],
    languageOptions: { globals: globals.node },
  },
  {
    // Build scripts run in Node and evaluate snippets inside a browser page.
    files: ['build/scripts/*.mjs'],
    languageOptions: { globals: { ...globals.node, ...globals.browser } },
  },
)
