/**
 * Generates the installer artwork from the Lumen logo (build/icon.png):
 *   build/icon.ico                       Windows icon (16 to 256 px)
 *   build/installerSidebar.bmp           NSIS welcome / finish page, 164x314
 *   build/uninstallerSidebar.bmp         same for the uninstaller
 *   build/installerHeader.bmp            NSIS header, 150x57
 *   build/dmg-background.png (+ @2x)     macOS disk image window, 660x400
 *
 * Pages are rendered with Playwright's Chromium; BMPs are written by hand (24-bit, no deps).
 * Run: node build/scripts/generate-installer-assets.mjs
 */
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from '@playwright/test'

const BUILD = join(dirname(fileURLToPath(import.meta.url)), '..')
const logo = `data:image/png;base64,${readFileSync(join(BUILD, 'icon.png')).toString('base64')}`

const NAVY_TOP = '#0f1426'
const NAVY_BOTTOM = '#070a14'
const FONT = "'Segoe UI Variable Display','Segoe UI','Helvetica Neue',Arial,sans-serif"

const sidebarHtml = `<body style="margin:0;width:164px;height:314px;overflow:hidden;
  background:linear-gradient(180deg,${NAVY_TOP},${NAVY_BOTTOM});font-family:${FONT};color:#fff;position:relative">
  <div style="position:absolute;left:-40px;top:-30px;width:200px;height:200px;border-radius:50%;
    background:radial-gradient(circle,rgba(150,140,255,.45),transparent 70%)"></div>
  <div style="position:absolute;right:-60px;bottom:-40px;width:200px;height:200px;border-radius:50%;
    background:radial-gradient(circle,rgba(90,200,250,.28),transparent 70%)"></div>
  <img src="${logo}" style="position:absolute;left:34px;top:70px;width:96px;height:96px;border-radius:22px;
    box-shadow:0 10px 28px rgba(0,0,0,.5)">
  <div style="position:absolute;left:0;right:0;top:186px;text-align:center;font-size:30px;font-weight:700;
    letter-spacing:-.5px">Lumen</div>
  <div style="position:absolute;left:0;right:0;top:228px;text-align:center;font-size:11px;
    color:rgba(255,255,255,.55)">Jellyfin, beautifully.</div></body>`

const headerHtml = `<body style="margin:0;width:150px;height:57px;overflow:hidden;background:#fff;
  font-family:${FONT};display:flex;align-items:center;justify-content:flex-end;gap:8px;padding-right:10px;box-sizing:border-box">
  <span style="font-size:17px;font-weight:700;color:#0f1426;letter-spacing:-.3px">Lumen</span>
  <img src="${logo}" style="width:40px;height:40px;border-radius:9px"></body>`

const dmgHtml = (w, h) => `<body style="margin:0;width:${w}px;height:${h}px;overflow:hidden;
  background:linear-gradient(180deg,${NAVY_TOP},${NAVY_BOTTOM});font-family:${FONT};color:#fff;position:relative">
  <div style="position:absolute;left:-80px;top:-100px;width:420px;height:420px;border-radius:50%;
    background:radial-gradient(circle,rgba(150,140,255,.35),transparent 70%)"></div>
  <div style="position:absolute;right:-100px;bottom:-120px;width:420px;height:420px;border-radius:50%;
    background:radial-gradient(circle,rgba(90,200,250,.22),transparent 70%)"></div>
  <div style="position:absolute;left:0;right:0;top:34px;text-align:center;font-size:22px;font-weight:600">
    Drag Lumen to your Applications folder</div>
  <svg style="position:absolute;left:250px;top:170px" width="160" height="60" viewBox="0 0 160 60" fill="none"
    stroke="rgba(255,255,255,.7)" stroke-width="4" stroke-linecap="round" stroke-linejoin="round">
    <path d="M10 30 H140"/><path d="M118 10 L142 30 L118 50"/></svg></body>`

/** Window size in CSS px; the 2x variant is rendered with deviceScaleFactor 2. */
async function render(browser, html, width, height, scale = 1) {
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: scale })
  await page.setContent(html)
  await page.waitForFunction(() => [...document.images].every((i) => i.complete))
  const png = await page.screenshot({ type: 'png' })
  await page.close()
  return png
}

/** PNG bytes -> raw RGBA through a canvas (keeps this script free of image dependencies). */
async function decode(browser, png) {
  const page = await browser.newPage()
  const out = await page.evaluate(async (b64) => {
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
  await page.close()
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

/** ICO container holding PNG images (supported since Windows Vista). */
function encodeIco(images) {
  const head = Buffer.alloc(6 + images.length * 16)
  head.writeUInt16LE(1, 2)
  head.writeUInt16LE(images.length, 4)
  let offset = head.length
  images.forEach(({ size, png }, i) => {
    const e = 6 + i * 16
    head[e] = size >= 256 ? 0 : size
    head[e + 1] = size >= 256 ? 0 : size
    head.writeUInt16LE(1, e + 4)
    head.writeUInt16LE(32, e + 6)
    head.writeUInt32LE(png.length, e + 8)
    head.writeUInt32LE(offset, e + 12)
    offset += png.length
  })
  return Buffer.concat([head, ...images.map((i) => i.png)])
}

/** Playwright's own Chromium when installed, else the system Edge / Chrome. */
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

const browser = await launch()
try {
  const sizes = [16, 24, 32, 48, 64, 128, 256]
  const icons = []
  for (const size of sizes) {
    const html = `<body style="margin:0;background:transparent"><img src="${logo}" style="width:${size}px;height:${size}px;display:block"></body>`
    const page = await browser.newPage({ viewport: { width: size, height: size } })
    await page.setContent(html)
    await page.waitForFunction(() => [...document.images].every((i) => i.complete))
    icons.push({ size, png: await page.screenshot({ type: 'png', omitBackground: true }) })
    await page.close()
  }
  writeFileSync(join(BUILD, 'icon.ico'), encodeIco(icons))

  const sidebar = encodeBmp(await decode(browser, await render(browser, sidebarHtml, 164, 314)))
  writeFileSync(join(BUILD, 'installerSidebar.bmp'), sidebar)
  writeFileSync(join(BUILD, 'uninstallerSidebar.bmp'), sidebar)
  writeFileSync(
    join(BUILD, 'installerHeader.bmp'),
    encodeBmp(await decode(browser, await render(browser, headerHtml, 150, 57))),
  )
  writeFileSync(
    join(BUILD, 'dmg-background.png'),
    await render(browser, dmgHtml(660, 400), 660, 400),
  )
  writeFileSync(
    join(BUILD, 'dmg-background@2x.png'),
    await render(browser, dmgHtml(660, 400), 660, 400, 2),
  )
  console.log('installer assets written to build/')
} finally {
  await browser.close()
}
