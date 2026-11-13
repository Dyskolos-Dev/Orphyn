/**
 * Renders the Windows installer / uninstaller artwork with Chromium (Segoe UI Variable text, glass
 * logo, rounded pills) into build/installer-art/:
 *   <page>-<lang><scale>.bmp           full page background, texts and pills baked in
 *   <page>-<lang><scale>-<crop>.bmp    small overlays (hover pills, unchecked checkbox rows)
 *   logo<scale>-<n>.bmp                the breathing logo animation, 12 frames
 *   installer-assets.nsh               file lists + click zones, included by build/installer.nsh
 * Window client area is 560x360 px at scale 100; scales 100, 125, 150 and 200 are generated.
 * Run by `npm run dist` (predist). The output is not committed.
 */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from '@playwright/test'

const BUILD = join(dirname(fileURLToPath(import.meta.url)), '..')
const OUT = join(BUILD, 'installer-art')
mkdirSync(OUT, { recursive: true })
const logo = `data:image/png;base64,${readFileSync(join(BUILD, 'icon.png')).toString('base64')}`
const FONT = "'Segoe UI Variable Display','Segoe UI Variable Text','Segoe UI',Arial,sans-serif"
const SCALES = [100, 125, 150, 200]
const FRAMES = 12
const W = 560
const H = 360

/** Layout in px at scale 100 (shared by the artwork and, through the generated .nsh, the zones). */
const L = {
  logo: { size: 96, x: 232, y: 30 },
  logoSmall: { size: 52, x: 254, y: 18 },
  frame: { x: 200, y: 6, w: 160, h: 138 }, // animated crop around the big logo
  title: { y: 148 },
  sub: { y: 196 },
  pill: { x: 185, y: 236, w: 190, h: 42 },
  link: { x: 230, y: 296, w: 100, h: 24 },
  card: { x: 50, y: 98, w: 460, h: 192 },
  well: { x: 68, y: 134, w: 300, h: 34 },
  browse: { x: 380, y: 134, w: 112, h: 34 },
  rows: [184, 214, 244].map((y) => ({ x: 68, y, w: 424, h: 26 })),
  install: { x: 180, y: 304, w: 200, h: 40 },
  back: { x: 12, y: 322, w: 90, h: 26 },
  bar: { x: 130, y: 214, w: 300, h: 4 },
}

const T = {
  en: {
    tagline: 'Your movies, shows and music, only better.',
    install: 'Install',
    options: 'Options',
    folder: 'Install folder',
    browse: 'Browse',
    rows: ['Desktop shortcut', 'Start menu shortcut', 'Launch Lumen when finished'],
    back: 'Back',
    installing: 'Installing Lumen…',
    ready: 'Lumen is ready',
    readySub: 'Everything is in place.',
    launch: 'Launch Lumen',
    done: 'Done',
    unTitle: 'Uninstall Lumen',
    unSub: 'Lumen will be removed from this computer.\nYour settings are kept.',
    unBtn: 'Uninstall',
    cancel: 'Cancel',
    uninstalling: 'Uninstalling Lumen…',
    removed: 'Lumen has been removed',
    removedSub: 'See you soon.',
  },
  fr: {
    tagline: 'Vos films, séries et musiques, en mieux.',
    install: 'Installer',
    options: 'Options',
    folder: "Dossier d'installation",
    browse: 'Parcourir',
    rows: ['Raccourci sur le Bureau', 'Raccourci dans le menu Démarrer', 'Lancer Lumen à la fin'],
    back: 'Retour',
    installing: 'Installation de Lumen…',
    ready: 'Lumen est prêt',
    readySub: 'Tout est en place.',
    launch: 'Lancer Lumen',
    done: 'Terminer',
    unTitle: 'Désinstaller Lumen',
    unSub: 'Lumen va être supprimé de cet ordinateur.\nVos réglages sont conservés.',
    unBtn: 'Désinstaller',
    cancel: 'Annuler',
    uninstalling: 'Désinstallation de Lumen…',
    removed: 'Lumen a été supprimé',
    removedSub: 'À bientôt.',
  },
}

const box = (r, extra = '') =>
  `position:absolute;left:${r.x}px;top:${r.y}px;width:${r.w}px;height:${r.h}px;${extra}`

const pill = (r, label, hover = false, secondary = false) =>
  `<div style="${box(
    r,
    `border-radius:${r.h / 2}px;display:flex;align-items:center;justify-content:center;
    font-size:${secondary ? 13 : 15}px;font-weight:600;color:#fff;letter-spacing:.1px;
    background:${
      secondary
        ? `rgba(255,255,255,${hover ? 0.2 : 0.11})`
        : hover
          ? 'linear-gradient(180deg,#b3a9ff,#8173f6)'
          : 'linear-gradient(180deg,#9a8dff,#6c5df2)'
    };
    box-shadow:${
      secondary
        ? 'inset 0 0 0 1px rgba(255,255,255,.14)'
        : `0 ${hover ? 10 : 7}px ${hover ? 30 : 22}px rgba(124,108,240,${hover ? 0.6 : 0.42}),inset 0 1px 0 rgba(255,255,255,.4)`
    }`,
  )}">${label}</div>`

const text = (y, size, weight, color, html, x = 0, w = W) =>
  `<div style="position:absolute;left:${x}px;top:${y}px;width:${w}px;text-align:center;
  font-size:${size}px;font-weight:${weight};color:${color};line-height:1.3;white-space:pre-line">${html}</div>`

const link = (label) => text(L.link.y + 2, 13, 500, 'rgba(255,255,255,.6)', label)

const check = (r, label, on) => `<div style="${box(r, 'display:flex;align-items:center;gap:11px')}">
  <div style="width:20px;height:20px;border-radius:6px;box-sizing:border-box;display:flex;align-items:center;
    justify-content:center;${
      on
        ? 'background:linear-gradient(180deg,#9a8dff,#6c5df2);box-shadow:0 2px 8px rgba(124,108,240,.45)'
        : 'box-shadow:inset 0 0 0 1.5px rgba(255,255,255,.35)'
    }">${
      on
        ? '<svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 6.4 4.8 9 10 3.2"/></svg>'
        : ''
    }</div>
  <span style="font-size:13.5px;color:#f5f5f7">${label}</span></div>`

/** The logo with its soft halo; phase is 0..FRAMES-1 for the breathing loop. */
function logoHtml(size, x, y, phase = 0) {
  const a = (phase / FRAMES) * Math.PI * 2
  const dy = -Math.sin(a) * 4
  const glow = 0.3 + 0.12 * (1 + Math.sin(a))
  return `<div style="position:absolute;left:${x - 30}px;top:${y - 30 + dy}px;width:${size + 60}px;height:${size + 60}px;
    border-radius:50%;background:radial-gradient(circle,rgba(150,140,255,${glow}),transparent 68%)"></div>
  <img src="${logo}" style="position:absolute;left:${x}px;top:${y + dy}px;width:${size}px;height:${size}px;
    border-radius:${size * 0.23}px;box-shadow:0 14px 36px rgba(0,0,0,.55),0 0 0 1px rgba(255,255,255,.1)">`
}

const glows = `
  <div style="position:absolute;left:-120px;top:-150px;width:460px;height:460px;border-radius:50%;
    background:radial-gradient(circle,rgba(150,140,255,.34),transparent 68%)"></div>
  <div style="position:absolute;right:-130px;bottom:-170px;width:480px;height:480px;border-radius:50%;
    background:radial-gradient(circle,rgba(90,200,250,.22),transparent 68%)"></div>
  <div style="position:absolute;left:300px;top:-90px;width:260px;height:260px;border-radius:50%;
    background:radial-gradient(circle,rgba(255,150,200,.15),transparent 70%)"></div>`

/**
 * One page. `o` selects variants: hover (which pill is lit), off (unchecked rows), phase (logo),
 * noPill (finish: the pill is a separate overlay because its label depends on an option).
 */
function page(name, lang, o = {}) {
  const t = T[lang]
  let body = ''
  switch (name) {
    case 'welcome':
      body =
        logoHtml(L.logo.size, L.logo.x, L.logo.y, o.phase) +
        text(L.title.y, 32, 700, '#f5f5f7', 'Lumen') +
        text(L.sub.y, 14.5, 400, 'rgba(255,255,255,.62)', t.tagline) +
        pill(L.pill, t.install, o.hover === 'pill') +
        link(t.options)
      break
    case 'unwelcome':
      body =
        logoHtml(L.logo.size, L.logo.x, L.logo.y, o.phase) +
        text(L.title.y - 6, 28, 700, '#f5f5f7', t.unTitle) +
        text(L.sub.y - 14, 14, 400, 'rgba(255,255,255,.62)', t.unSub) +
        pill(L.pill, t.unBtn, o.hover === 'pill') +
        link(t.cancel)
      break
    case 'options':
      body =
        logoHtml(L.logoSmall.size, L.logoSmall.x, L.logoSmall.y, 0) +
        text(76, 17, 650, '#f5f5f7', t.options) +
        `<div style="${box(L.card, 'border-radius:22px;background:rgba(255,255,255,.06);box-shadow:inset 0 0 0 1px rgba(255,255,255,.09)')}"></div>` +
        `<div style="position:absolute;left:68px;top:112px;font-size:12px;color:rgba(255,255,255,.55)">${t.folder}</div>` +
        `<div style="${box(L.well, 'border-radius:11px;background:#161b2f;box-shadow:inset 0 0 0 1px rgba(255,255,255,.08)')}"></div>` +
        pill(L.browse, t.browse, o.hover === 'browse', true) +
        L.rows.map((r, i) => check(r, t.rows[i], !(o.off ?? []).includes(i))).join('') +
        pill(L.install, t.install, o.hover === 'install') +
        `<div style="${box(L.back, 'display:flex;align-items:center;padding-left:6px;font-size:12.5px;color:rgba(255,255,255,.6)')}">‹&nbsp;${t.back}</div>`
      break
    case 'progress':
      body =
        logoHtml(L.logo.size, L.logo.x, L.logo.y, 0) +
        text(L.title.y + 2, 24, 650, '#f5f5f7', t.installing)
      break
    case 'unprogress':
      body =
        logoHtml(L.logo.size, L.logo.x, L.logo.y, 0) +
        text(L.title.y + 2, 24, 650, '#f5f5f7', t.uninstalling)
      break
    case 'finish':
      body =
        logoHtml(L.logo.size, L.logo.x, L.logo.y, o.phase) +
        text(L.title.y - 4, 28, 700, '#f5f5f7', t.ready) +
        text(L.sub.y - 8, 14.5, 400, 'rgba(255,255,255,.62)', t.readySub) +
        (o.pill ? pill(L.pill, o.pill === 'launch' ? t.launch : t.done, o.hover === 'pill') : '')
      break
    case 'unfinish':
      body =
        logoHtml(L.logo.size, L.logo.x, L.logo.y, o.phase) +
        text(L.title.y - 4, 28, 700, '#f5f5f7', t.removed) +
        text(L.sub.y - 8, 14.5, 400, 'rgba(255,255,255,.62)', t.removedSub) +
        pill(L.pill, t.done, o.hover === 'pill')
      break
  }
  return `<body style="margin:0;width:${W}px;height:${H}px;overflow:hidden;position:relative;
    background:linear-gradient(180deg,#10162a,#0b0f1c 55%,#070a14);font-family:${FONT}">${glows}${body}</body>`
}

/** PNG bytes -> raw RGBA through a canvas (keeps this script free of image dependencies). */
async function decode(dpage, png) {
  const out = await dpage.evaluate(async (b64) => {
    const img = new Image()
    img.src = `data:image/png;base64,${b64}`
    await img.decode()
    const c = document.createElement('canvas')
    c.width = img.width
    c.height = img.height
    const ctx = c.getContext('2d')
    ctx.drawImage(img, 0, 0)
    const d = ctx.getImageData(0, 0, c.width, c.height).data
    let s = ''
    for (let i = 0; i < d.length; i += 8192) s += String.fromCharCode(...d.subarray(i, i + 8192))
    return { w: c.width, h: c.height, data: btoa(s) }
  }, png.toString('base64'))
  return { width: out.w, height: out.h, rgba: Buffer.from(out.data, 'base64') }
}

/** 24-bit uncompressed BMP, bottom-up rows padded to 4 bytes. */
function encodeBmp({ width, height, rgba }) {
  const rowSize = Math.ceil((width * 3) / 4) * 4
  const pixels = rowSize * height
  const buf = Buffer.alloc(54 + pixels)
  buf.write('BM', 0)
  buf.writeUInt32LE(buf.length, 2)
  buf.writeUInt32LE(54, 10)
  buf.writeUInt32LE(40, 14)
  buf.writeInt32LE(width, 18)
  buf.writeInt32LE(height, 22)
  buf.writeUInt16LE(1, 26)
  buf.writeUInt16LE(24, 28)
  buf.writeUInt32LE(pixels, 34)
  for (let y = 0; y < height; y++) {
    const dst = 54 + (height - 1 - y) * rowSize
    for (let x = 0; x < width; x++) {
      const src = (y * width + x) * 4
      buf[dst + x * 3] = rgba[src + 2]
      buf[dst + x * 3 + 1] = rgba[src + 1]
      buf[dst + x * 3 + 2] = rgba[src]
    }
  }
  return buf
}

async function launch() {
  for (const channel of [undefined, 'msedge', 'chrome']) {
    try {
      return await chromium.launch({ channel })
    } catch {
      /* try the next one */
    }
  }
  throw new Error('No Chromium found: run "npx playwright install chromium"')
}

// Overlays include the pill's whole drop shadow so their edges never show; clamped to the window.
const pad = 36
const grow = (r) => {
  const x = Math.max(0, r.x - pad)
  const y = Math.max(0, r.y - pad)
  return { x, y, w: Math.min(W, r.x + r.w + pad) - x, h: Math.min(H, r.y + r.h + pad) - y }
}

/** name -> { page, lang options, clip } for every overlay of a page. */
const CROPS = {
  welcome: [['pillh', { hover: 'pill' }, grow(L.pill)]],
  unwelcome: [['pillh', { hover: 'pill' }, grow(L.pill)]],
  unfinish: [['pillh', { hover: 'pill' }, grow(L.pill)]],
  finish: [
    ['launch', { pill: 'launch' }, grow(L.pill)],
    ['launchh', { pill: 'launch', hover: 'pill' }, grow(L.pill)],
    ['done', { pill: 'done' }, grow(L.pill)],
    ['doneh', { pill: 'done', hover: 'pill' }, grow(L.pill)],
  ],
  options: [
    ['browseh', { hover: 'browse' }, grow(L.browse)],
    ['installh', { hover: 'install' }, grow(L.install)],
    ...L.rows.map((r, i) => [`off${i}`, { off: [i] }, r]),
  ],
}
const PAGES = ['welcome', 'options', 'progress', 'finish', 'unwelcome', 'unprogress', 'unfinish']

const browser = await launch()
try {
  const dpage = await browser.newPage()
  for (const scale of SCALES) {
    const page_ = await browser.newPage({
      viewport: { width: W, height: H },
      deviceScaleFactor: scale / 100,
    })
    const save = async (file, html, clip) => {
      await page_.setContent(html)
      await page_.waitForFunction(() => [...document.images].every((i) => i.complete))
      const png = await page_.screenshot({
        type: 'png',
        ...(clip ? { clip: { x: clip.x, y: clip.y, width: clip.w, height: clip.h } } : {}),
      })
      writeFileSync(join(OUT, `${file}.bmp`), encodeBmp(await decode(dpage, png)))
    }
    for (const lang of ['en', 'fr']) {
      for (const name of PAGES) {
        // The finish page's pill is an overlay (its label depends on the options page).
        await save(`${name}-${lang}${scale}`, page(name, lang))
        for (const [crop, o, clip] of CROPS[name] ?? []) {
          await save(`${name}-${lang}${scale}-${crop}`, page(name, lang, o), clip)
        }
      }
    }
    for (let k = 0; k < FRAMES; k++) {
      await save(`logo${scale}-${k}`, page('welcome', 'en', { phase: k }), L.frame)
    }
    await page_.close()
  }
} finally {
  await browser.close()
}

// ---- installer-assets.nsh: File lists (compile-time names, runtime branch) and click zones ----
const BS = '\\'
let nsh = '; Generated by build/scripts/generate-installer-art.mjs. Do not edit.\n'
const zone = (n, r) =>
  `!define LZ_${n}_X1 ${r.x}\n!define LZ_${n}_Y1 ${r.y}\n!define LZ_${n}_X2 ${r.x + r.w}\n!define LZ_${n}_Y2 ${r.y + r.h}\n`
nsh += `!define LUMEN_W ${W}\n!define LUMEN_H ${H}\n`
nsh +=
  zone('PILL', L.pill) +
  zone('LINK', L.link) +
  zone('BROWSE', L.browse) +
  zone('INSTALL', L.install)
nsh +=
  zone('BACK', L.back) + zone('ROW0', L.rows[0]) + zone('ROW1', L.rows[1]) + zone('ROW2', L.rows[2])
nsh += zone('WELL', L.well) + zone('BAR', L.bar) + zone('FRAME', L.frame)
const gz = (n, r) =>
  `!define LZ_${n}_X ${r.x}\n!define LZ_${n}_Y ${r.y}\n!define LZ_${n}_W ${r.w}\n!define LZ_${n}_H ${r.h}\n`
nsh += gz('GPILL', grow(L.pill)) + gz('GBROWSE', grow(L.browse)) + gz('GINSTALL', grow(L.install))
nsh += gz('GROW0', L.rows[0]) + gz('GROW1', L.rows[1]) + gz('GROW2', L.rows[2])
nsh += gz('GFRAME', L.frame)
const fileLine = (src, dst) =>
  `      File "/oname=$PLUGINSDIR${BS}${dst}.bmp" "\${BUILD_RESOURCES_DIR}${BS}installer-art${BS}${src}.bmp"\n`
for (const name of PAGES) {
  const crops = (CROPS[name] ?? []).map((c) => c[0])
  nsh += `!macro LUMEN_FILES_${name}\n`
  for (const lang of ['en', 'fr']) {
    for (const scale of SCALES) {
      nsh += `  \${If} $LumenKey == "${lang}${scale}"\n`
      nsh += fileLine(`${name}-${lang}${scale}`, 'bg')
      for (const c of crops) nsh += fileLine(`${name}-${lang}${scale}-${c}`, c)
      nsh += '  ${EndIf}\n'
    }
  }
  nsh += '!macroend\n'
}
nsh += '!macro LUMEN_FILES_logo\n'
for (const scale of SCALES) {
  nsh += `  \${If} $LumenBucket == ${scale}\n`
  for (let k = 0; k < FRAMES; k++) nsh += fileLine(`logo${scale}-${k}`, `logo${k}`)
  nsh += '  ${EndIf}\n'
}
nsh += '!macroend\n'
nsh += `!define LUMEN_FRAMES ${FRAMES}\n`
writeFileSync(join(OUT, 'installer-assets.nsh'), nsh)
console.log('installer art written to build/installer-art/')
