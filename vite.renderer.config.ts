/**
 * Plain Vite config for the renderer alone, used by the Tauri shell (`npm run tauri:dev` /
 * `tauri:build`). Same root, plugins and aliases as the `renderer` section of
 * `electron.vite.config.ts`. Output: `dist/renderer/` (already ignored by git and ESLint).
 * Dev port: `ORPHYN_DEV_PORT` (default 1420, the `devUrl` of `src-tauri/tauri.conf.json`).
 */
import { resolve } from 'node:path'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

const port = Number(process.env['ORPHYN_DEV_PORT']) || 1420

export default defineConfig({
  root: resolve(__dirname, 'src/renderer'),
  base: './',
  resolve: {
    alias: {
      '@renderer': resolve(__dirname, 'src/renderer/src'),
      '@shared': resolve(__dirname, 'src/shared'),
    },
  },
  plugins: [react()],
  // Tauri prints its own logs; keep Vite's output and errors visible.
  clearScreen: false,
  server: {
    port,
    strictPort: true,
    host: 'localhost',
  },
  build: {
    outDir: resolve(__dirname, 'dist/renderer'),
    emptyOutDir: true,
    // WebView2 is evergreen Chromium; WKWebView follows Safari 16+ (macOS 13+); WebKitGTK 2.40+.
    target: ['chrome110', 'safari16'],
    sourcemap: false,
  },
})
