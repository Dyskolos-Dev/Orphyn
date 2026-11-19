// Writes latest.json from the release's .sig files (the native build jobs do not write one), so every
// platform is listed. An existing latest.json in the folder is completed instead.
// Usage: node scripts/complete-updater-json.mjs <dir with latest.json and *.sig> <tag>
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const [dir, tag] = process.argv.slice(2)
const base = `https://github.com/Dyskolos-Dev/Orphyn/releases/download/${tag}/`
const manifestPath = join(dir, 'latest.json')
const manifest = existsSync(manifestPath)
  ? JSON.parse(readFileSync(manifestPath, 'utf8'))
  : {
      version: tag.replace(/^v/, ''),
      notes: 'See CHANGELOG.md for what is new in this version.',
      pub_date: new Date().toISOString(),
      platforms: {},
    }
const rules = [
  [/_x64-setup\.exe\.sig$/, ['windows-x86_64', 'windows-x86_64-nsis']],
  [/_x64\.app\.tar\.gz\.sig$/, ['darwin-x86_64', 'darwin-x86_64-app']],
  [/_aarch64\.app\.tar\.gz\.sig$/, ['darwin-aarch64', 'darwin-aarch64-app']],
  [/_amd64\.AppImage\.sig$/, ['linux-x86_64', 'linux-x86_64-appimage']],
  [/_amd64\.deb\.sig$/, ['linux-x86_64-deb']],
]
for (const file of readdirSync(dir).filter((f) => f.endsWith('.sig'))) {
  const keys = rules.find(([re]) => re.test(file))?.[1] ?? []
  const entry = {
    signature: readFileSync(join(dir, file), 'utf8').trim(),
    url: base + file.slice(0, -4),
  }
  for (const key of keys) manifest.platforms[key] = entry
}
writeFileSync(join(dir, 'latest.json'), JSON.stringify(manifest, null, 2) + '\n')
console.log('latest.json platforms:', Object.keys(manifest.platforms).join(', '))
