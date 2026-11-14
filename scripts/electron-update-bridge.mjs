/**
 * Update bridge for the 1.0.x clients (Electron). They update through electron-updater, which
 * reads `latest.yml` (Windows), `latest-linux.yml` and `latest-mac.yml` from the latest GitHub
 * release. This script writes those three files for the Tauri installers of a release, so 1.0.x
 * clients move to 1.1+ by themselves:
 *
 * - Windows: `latest.yml` points to the NSIS setup (`Orphyn_<v>_x64-setup.exe`). electron-updater
 *   downloads it, checks its sha512, and runs it with `--updated` (plus `/S` when applied as the
 *   app quits). The setup removes the Electron install (build/windows/hooks.nsh) and starts the new
 *   app; the servers are imported from the old profile and ask to sign in again.
 * - Linux AppImage: `latest-linux.yml` points to the new AppImage, which electron-updater
 *   downloads, checks and puts in place of the old one. (.deb installs only show "update
 *   available" and open the release page, as before.)
 * - macOS: 1.0.x never installed updates in the app (unsigned builds): `latest-mac.yml` makes it
 *   show "update available", and the button opens the release page to download the .dmg once.
 *
 * Usage: node scripts/electron-update-bridge.mjs <assets dir> [version]
 * The version defaults to package.json's. The files are written into <assets dir>.
 */
import { createHash } from 'node:crypto'
import { createReadStream, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')

function sha512(path) {
  return new Promise((resolve, reject) => {
    const hash = createHash('sha512')
    createReadStream(path)
      .on('data', (chunk) => hash.update(chunk))
      .on('end', () => resolve(hash.digest('base64')))
      .on('error', reject)
  })
}

async function describe(dir, name) {
  const path = join(dir, name)
  return { url: name, sha512: await sha512(path), size: statSync(path).size }
}

/** electron-updater's YAML: a flat document, every value quoted where needed. */
function toYaml(version, files, releaseDate) {
  let out = `version: ${version}\nfiles:\n`
  for (const f of files) {
    out += `  - url: ${f.url}\n    sha512: ${f.sha512}\n    size: ${f.size}\n`
  }
  out += `path: ${files[0].url}\nsha512: ${files[0].sha512}\nreleaseDate: '${releaseDate}'\n`
  return out
}

/** Picks the one file matching `re`; exits when there is none (or several, unless `many`). */
function pick(names, re, what, many = false) {
  const found = names.filter((n) => re.test(n))
  if (found.length === 0) return []
  if (!many && found.length > 1) {
    console.error(`several ${what} found: ${found.join(', ')}`)
    process.exit(1)
  }
  return found
}

export async function writeBridge(dir, version, releaseDate = new Date().toISOString()) {
  const names = readdirSync(dir)
  const v = version.replace(/[.+]/g, (c) => `\\${c}`)
  const written = []
  const targets = [
    // NSIS setup only (never the .nsis.zip or .sig of the Tauri updater).
    ['latest.yml', pick(names, new RegExp(`^Orphyn_${v}_x64-setup\\.exe$`), 'Windows setup')],
    ['latest-linux.yml', pick(names, new RegExp(`^Orphyn_${v}_amd64\\.AppImage$`), 'AppImage')],
    // Apple silicon first: electron-updater falls back to the first file.
    [
      'latest-mac.yml',
      pick(names, new RegExp(`^Orphyn_${v}_(aarch64|x64)\\.dmg$`), 'dmg', true).sort(),
    ],
  ]
  for (const [file, assets] of targets) {
    if (assets.length === 0) {
      console.warn(`${file}: no matching installer in ${dir}, skipped`)
      continue
    }
    const files = await Promise.all(assets.map((a) => describe(dir, a)))
    writeFileSync(join(dir, file), toYaml(version, files, releaseDate))
    written.push(file)
  }
  return written
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const dir = process.argv[2]
  if (!dir) {
    console.error('usage: node scripts/electron-update-bridge.mjs <assets dir> [version]')
    process.exit(1)
  }
  const version =
    process.argv[3]?.replace(/^v/, '') ||
    JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8')).version
  const written = await writeBridge(dir, version)
  if (written.length === 0) {
    console.error('no update file written')
    process.exit(1)
  }
  console.log(`wrote ${written.join(', ')} for ${version}`)
}
