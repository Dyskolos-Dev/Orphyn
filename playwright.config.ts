import { defineConfig } from '@playwright/test'

/**
 * End-to-end suite: drives the Tauri debug build (dist/tauri-target/debug/Orphyn.exe) over the
 * WebView2 DevTools protocol against a real Jellyfin server (Windows). Run with `npm run e2e`;
 * it needs ORPHYN_E2E_URL / ORPHYN_E2E_USER / ORPHYN_E2E_PASSWORD and skips every scenario without them. See docs/QA.md.
 */
export default defineConfig({
  testDir: './e2e',
  globalSetup: './e2e/global-setup.ts',
  outputDir: './e2e/test-results',
  // One app instance at a time: scenarios share a profile and a server-side session.
  workers: 1,
  fullyParallel: false,
  retries: 0,
  timeout: 180_000,
  expect: { timeout: 15_000 },
  reporter: [['list']],
  use: {
    trace: 'off',
    screenshot: 'off',
  },
})
