/**
 * `npm run dist`: release build and installers of the native app for the current OS, written to
 * `dist/native/` with the names the 1.5 updater (tauri-plugin-updater) and the 1.0.x bridge expect:
 *
 *   Windows  Orphyn_<v>_x64-setup.exe (+ .sig)        NSIS, build/windows/installer.nsi, libmpv-2.dll beside the exe
 *   macOS    Orphyn_<v>_<arch>.dmg, Orphyn_<arch>.app.tar.gz (+ .sig)   ad-hoc signed .app, libmpv dylibs bundled
 *   Linux    Orphyn_<v>_amd64.AppImage (+ .sig), Orphyn_<v>_amd64.deb (+ .sig)
 *
 * The updater signatures (minisign) come from `tauri signer sign` with TAURI_SIGNING_PRIVATE_KEY
 * (CI secret) or `~/.orphyn/updater.key`; without a key the installers are built unsigned (warning)
 * unless --require-sign is given.
 *
 * Version: package.json is the source; native/crates/orphyn-native/Cargo.toml must carry the same one.
 * Options: --target <rust triple> (macOS), --no-build, --require-sign.
 * Inputs: libmpv. Windows: ORPHYN_MPV_DIR = folder holding libmpv-2.dll. macOS: brew's `mpv`
 * (MPV_PREFIX to override). Linux: libmpv.so.2 from libmpv2 (+ LINUXDEPLOY = linuxdeploy AppImage).
 */
import { spawnSync } from 'node:child_process'
import {
  chmodSync,
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { homedir } from 'node:os'
import { basename, dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const NATIVE = join(ROOT, 'native')
const OUT = join(ROOT, 'dist', 'native')
const args = process.argv.slice(2)
const flag = (n) => args.includes(n)
const opt = (n) => (args.includes(n) ? args[args.indexOf(n) + 1] : undefined)

const pkg = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8'))
const VERSION = pkg.version
const crateVersion = /^version\s*=\s*"([^"]+)"/m.exec(
  readFileSync(join(NATIVE, 'crates/orphyn-native/Cargo.toml'), 'utf8'),
)?.[1]
if (crateVersion !== VERSION) {
  fail(`package.json is ${VERSION} but native/crates/orphyn-native/Cargo.toml is ${crateVersion}`)
}
const PRODUCT = 'Orphyn'
const BUNDLE_ID = 'app.lumen.desktop'
const HOME_PAGE = pkg.homepage
const PUBLISHER = 'Dyskolos_'
const DESCRIPTION = 'A desktop Jellyfin client for music, movies and series'

function fail(msg) {
  console.error(`native-package: ${msg}`)
  process.exit(1)
}
function run(cmd, cmdArgs, o = {}) {
  const r = spawnSync(cmd, cmdArgs, { stdio: 'inherit', ...o })
  if (r.status !== 0) fail(`${cmd} ${cmdArgs.join(' ')} failed (${r.status ?? r.error})`)
}
function out(cmd, cmdArgs) {
  const r = spawnSync(cmd, cmdArgs, { encoding: 'utf8' })
  if (r.status !== 0) fail(`${cmd} ${cmdArgs.join(' ')} failed: ${r.stderr}`)
  return r.stdout
}
function dirSizeKb(dir) {
  let total = 0
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name)
    total += e.isDirectory() ? dirSizeKb(p) : statSync(p).size / 1024
  }
  return Math.ceil(total)
}

/** The small subset of Handlebars the installer / desktop templates use: {{var}}, {{#if v}}, flat {{#each v}}. */
function render(template, ctx) {
  let t = template.replace(/\r\n/g, '\n')
  t = t.replace(/[ \t]*\{\{#if (\w+)\}\}\n?([\s\S]*?)[ \t]*\{\{\/if\}\}\n?/g, (_, k, b) =>
    ctx[k] ? b : '',
  )
  t = t.replace(/[ \t]*\{\{#each (\w+)\}\}\n?([\s\S]*?)[ \t]*\{\{\/each\}\}\n?/g, (_, k, b) => {
    const v = ctx[k] ?? []
    const rows = Array.isArray(v) ? v.map((x) => [null, x]) : Object.entries(v)
    return rows
      .map(([key, val]) =>
        b
          .replace(/\{\{no-escape @key\}\}/g, key)
          .replace(/\{\{this\.\[1\]\}\}/g, val?.[1])
          .replace(/\{\{this\}\}/g, val),
      )
      .join('')
  })
  t = t.replace(/\{\{(\w+)\}\}/g, (_, k) => ctx[k] ?? '')
  if (t.includes('{{')) fail(`template left unresolved: ${/\{\{[^}]*\}\}/.exec(t)?.[0]}`)
  return t
}

/** Minisign signature for the in-app updater (tauri-plugin-updater). */
function sign(file) {
  const env = { ...process.env }
  if (!env.TAURI_SIGNING_PRIVATE_KEY) {
    const keyFile = join(homedir(), '.orphyn', 'updater.key')
    if (existsSync(keyFile)) {
      env.TAURI_SIGNING_PRIVATE_KEY = readFileSync(keyFile, 'utf8').trim()
    } else if (flag('--require-sign')) {
      fail('no updater signing key: set TAURI_SIGNING_PRIVATE_KEY or ~/.orphyn/updater.key')
    } else {
      console.warn(`warning: no updater signing key, ${basename(file)} is not signed`)
      return
    }
  }
  env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ??= ''
  run('npx', ['tauri', 'signer', 'sign', file], {
    cwd: ROOT,
    env,
    shell: process.platform === 'win32',
  })
}

function cargoBuild(target) {
  if (flag('--no-build')) return
  run(
    'cargo',
    ['build', '--release', '--locked', '-p', 'orphyn-native', ...(target ? ['--target', target] : [])],
    { cwd: NATIVE },
  )
}
const binPath = (target, name) =>
  join(NATIVE, 'target', target ?? '', 'release', name + (process.platform === 'win32' ? '.exe' : ''))

rmSync(OUT, { recursive: true, force: true })
mkdirSync(OUT, { recursive: true })

if (process.platform === 'win32') windows()
else if (process.platform === 'darwin') macos()
else linux()
console.log('\nAssets in', OUT, '\n' + readdirSync(OUT).filter((f) => !statSync(join(OUT, f)).isDirectory()).join('\n'))

// ---------------------------------------------------------------- Windows
function windows() {
  const mpvDir = process.env.ORPHYN_MPV_DIR
  const dll = mpvDir && join(mpvDir, 'libmpv-2.dll')
  if (!dll || !existsSync(dll)) fail('set ORPHYN_MPV_DIR to the folder holding libmpv-2.dll')
  cargoBuild()
  const stage = join(OUT, 'win')
  mkdirSync(stage, { recursive: true })
  copyFileSync(binPath(undefined, 'orphyn-native'), join(stage, `${PRODUCT}.exe`))
  copyFileSync(dll, join(stage, 'libmpv-2.dll'))

  const nsis = join(ROOT, 'build/windows')
  const file = `${PRODUCT}_${VERSION}_x64-setup.exe`
  const ctx = {
    compression: 'lzma',
    installer_hooks: join(nsis, 'hooks.nsh'),
    manufacturer: PUBLISHER,
    product_name: PRODUCT,
    version: VERSION,
    version_with_build: `${VERSION.replace(/[-+].*$/, '')}.0`,
    homepage: HOME_PAGE,
    install_mode: 'currentUser',
    installer_icon: join(ROOT, 'build/icon.ico'),
    main_binary_name: PRODUCT,
    main_binary_path: join(stage, `${PRODUCT}.exe`),
    bundle_id: BUNDLE_ID,
    copyright: `Copyright (c) ${PUBLISHER}`,
    out_file: join(OUT, file),
    arch: 'x64',
    additional_plugins_path: join(nsis, 'plugins'),
    allow_downgrades: 'true',
    install_webview2_mode: 'skip',
    estimated_size: dirSizeKb(stage),
    languages: ['English', 'French'],
    language_files: [join(nsis, 'languages/English.nsh'), join(nsis, 'languages/French.nsh')],
    resources: { [join(stage, 'libmpv-2.dll')]: ['', 'libmpv-2.dll'] },
  }
  const script = join(stage, 'installer.nsi')
  // utils.nsh and FileAssociation.nsh are included relative to the script.
  cpSync(join(nsis, 'include'), stage, { recursive: true })
  writeFileSync(script, '﻿' + render(readFileSync(join(nsis, 'installer.nsi'), 'utf8'), ctx))
  const tauriNsis = join(process.env.LOCALAPPDATA ?? '', 'tauri', 'NSIS', 'Bin', 'makensis.exe')
  const makensis =
    process.env.MAKENSIS ?? (existsSync(tauriNsis) ? tauriNsis : 'makensis')
  run(makensis, ['/V2', script])
  rmSync(stage, { recursive: true, force: true })
  sign(join(OUT, file))
}

// ---------------------------------------------------------------- macOS
function macos() {
  const triple = opt('--target') ?? (process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin')
  const arch = triple.startsWith('aarch64') ? 'aarch64' : 'x64'
  cargoBuild(triple)
  const app = join(OUT, `${PRODUCT}.app`)
  const macos = join(app, 'Contents/MacOS')
  const frameworks = join(app, 'Contents/Frameworks')
  mkdirSync(macos, { recursive: true })
  mkdirSync(frameworks, { recursive: true })
  mkdirSync(join(app, 'Contents/Resources'), { recursive: true })
  copyFileSync(binPath(triple, 'orphyn-native'), join(macos, PRODUCT))
  copyFileSync(join(ROOT, 'build/icons/icon.icns'), join(app, 'Contents/Resources/icon.icns'))
  writeFileSync(
    join(app, 'Contents/Info.plist'),
    `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>${PRODUCT}</string>
<key>CFBundleIdentifier</key><string>${BUNDLE_ID}</string>
<key>CFBundleName</key><string>${PRODUCT}</string>
<key>CFBundleDisplayName</key><string>${PRODUCT}</string>
<key>CFBundleIconFile</key><string>icon</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>${VERSION}</string>
<key>CFBundleVersion</key><string>${VERSION}</string>
<key>LSApplicationCategoryType</key><string>public.app-category.entertainment</string>
<key>LSMinimumSystemVersion</key><string>11.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSHumanReadableCopyright</key><string>Copyright (c) ${PUBLISHER}</string>
</dict></plist>
`,
  )

  // libmpv is loaded by name from the executable's folder; its own dependencies go to Frameworks.
  const prefix = process.env.MPV_PREFIX ?? out('brew', ['--prefix', 'mpv']).trim()
  const mpv = realpathSync(join(prefix, 'lib/libmpv.2.dylib'))
  const placed = join(macos, 'libmpv.2.dylib')
  copyFileSync(mpv, placed)
  chmodSync(placed, 0o755)
  run('install_name_tool', ['-id', '@loader_path/libmpv.2.dylib', placed])
  const libDirs = [dirname(mpv), join(prefix, 'lib'), '/opt/homebrew/lib', '/usr/local/lib']
  const copied = new Set()
  const bundle = (file, rel) => {
    const lines = out('otool', ['-L', file]).split('\n').slice(1)
    for (const line of lines) {
      const dep = line.trim().split(' (')[0]
      if (!dep || dep.startsWith('/usr/lib') || dep.startsWith('/System') || basename(dep) === basename(file)) continue
      const name = basename(dep)
      if (!copied.has(name)) {
        const src = [dep, ...libDirs.map((d) => join(d, name))].find((p) => p.startsWith('/') && existsSync(p))
        if (!src) fail(`cannot find ${dep} (needed by ${basename(file)})`)
        const dest = join(frameworks, name)
        copyFileSync(realpathSync(src), dest)
        chmodSync(dest, 0o755)
        copied.add(name)
        run('install_name_tool', ['-id', `@loader_path/${name}`, dest])
        bundle(dest, '@loader_path/')
      }
      run('install_name_tool', ['-change', dep, `${rel}${name}`, file])
    }
  }
  bundle(placed, '@loader_path/../Frameworks/')
  for (const f of readdirSync(frameworks)) run('codesign', ['--force', '--sign', '-', join(frameworks, f)])
  run('codesign', ['--force', '--sign', '-', placed])
  // Ad hoc signature of the whole bundle (no developer certificate): an unsigned or broken one
  // makes macOS report the app as "damaged" on Apple silicon.
  run('codesign', ['--force', '--deep', '--sign', '-', app])
  run('codesign', ['--verify', '--deep', '--strict', '--verbose=2', app])

  const tarball = `${PRODUCT}_${arch}.app.tar.gz`
  run('tar', ['-czf', join(OUT, tarball), '-C', OUT, `${PRODUCT}.app`])
  sign(join(OUT, tarball))

  const dmg = `${PRODUCT}_${VERSION}_${arch}.dmg`
  const src = join(OUT, 'dmg-src')
  mkdirSync(src, { recursive: true })
  cpSync(app, join(src, `${PRODUCT}.app`), { recursive: true, verbatimSymlinks: true })
  run('create-dmg', [
    '--volname', PRODUCT,
    '--background', join(ROOT, 'build/dmg-background.png'),
    '--window-size', '660', '400',
    '--icon-size', '100',
    '--icon', `${PRODUCT}.app`, '180', '200',
    '--app-drop-link', '480', '200',
    '--no-internet-enable',
    join(OUT, dmg),
    src,
  ])
  rmSync(src, { recursive: true, force: true })
}

// ---------------------------------------------------------------- Linux
function linux() {
  cargoBuild()
  const exe = binPath(undefined, 'orphyn-native')
  const desktopTemplate = readFileSync(join(ROOT, 'build/linux/orphyn.desktop'), 'utf8')
  const desktop = (exec) =>
    render(desktopTemplate, { comment: DESCRIPTION, exec, icon: 'orphyn', name: PRODUCT, mime_type: '' })

  // .deb: libmpv comes from the system (Depends), the app is /usr/bin/orphyn.
  const deb = `${PRODUCT}_${VERSION}_amd64.deb`
  const root = join(OUT, 'deb')
  mkdirSync(join(root, 'DEBIAN'), { recursive: true })
  mkdirSync(join(root, 'usr/bin'), { recursive: true })
  mkdirSync(join(root, 'usr/share/applications'), { recursive: true })
  mkdirSync(join(root, 'usr/share/icons/hicolor/512x512/apps'), { recursive: true })
  copyFileSync(exe, join(root, 'usr/bin/orphyn'))
  chmodSync(join(root, 'usr/bin/orphyn'), 0o755)
  writeFileSync(join(root, 'usr/share/applications/orphyn.desktop'), desktop('orphyn'))
  copyFileSync(join(ROOT, 'build/icons/icon-512.png'), join(root, 'usr/share/icons/hicolor/512x512/apps/orphyn.png'))
  writeFileSync(
    join(root, 'DEBIAN/control'),
    `Package: orphyn
Version: ${VERSION}
Architecture: amd64
Maintainer: ${PUBLISHER}
Section: video
Priority: optional
Homepage: ${HOME_PAGE}
Installed-Size: ${dirSizeKb(root)}
Depends: libmpv2, libasound2 | libasound2t64, libxkbcommon0, libwayland-client0, libvulkan1, libdbus-1-3, libfontconfig1, libx11-6
Recommends: gnome-keyring | kwalletmanager
Description: ${DESCRIPTION}
 Orphyn is a native desktop client for Jellyfin: music, movies and series.
`,
  )
  run('dpkg-deb', ['--build', '--root-owner-group', root, join(OUT, deb)])
  sign(join(OUT, deb))

  // AppImage: libmpv and its libraries are bundled (the app loads libmpv.so.2 by name, and the
  // AppImage's AppRun puts usr/lib on the library path).
  const linuxdeploy = process.env.LINUXDEPLOY
  if (!linuxdeploy) fail('set LINUXDEPLOY to the linuxdeploy AppImage')
  const appdir = join(OUT, 'AppDir')
  const mpvLib = ['/usr/lib/x86_64-linux-gnu/libmpv.so.2', '/usr/lib/libmpv.so.2'].find(existsSync)
  if (!mpvLib) fail('libmpv.so.2 not found (install libmpv2)')
  const dsk = join(OUT, 'orphyn.desktop')
  writeFileSync(dsk, desktop('orphyn-native'))
  run(
    linuxdeploy,
    ['--appdir', appdir, '--executable', exe, '--library', mpvLib, '--desktop-file', dsk,
      '--icon-file', join(ROOT, 'build/icons/icon-512.png'), '--icon-filename', 'orphyn', '--output', 'appimage'],
    { cwd: OUT, env: { ...process.env, APPIMAGE_EXTRACT_AND_RUN: '1', LINUXDEPLOY_OUTPUT_VERSION: VERSION } },
  )
  const built = readdirSync(OUT).find((f) => f.endsWith('.AppImage'))
  if (!built) fail('linuxdeploy produced no AppImage')
  const image = `${PRODUCT}_${VERSION}_amd64.AppImage`
  copyFileSync(join(OUT, built), join(OUT, image))
  if (built !== image) rmSync(join(OUT, built))
  sign(join(OUT, image))
  for (const d of ['deb', 'AppDir']) rmSync(join(OUT, d), { recursive: true, force: true })
  rmSync(dsk)
}
