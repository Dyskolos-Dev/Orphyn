/**
 * Manual check tool (no Electron needed). See docs/CASTING.md ("Manual check").
 *
 *   node out/cast-cli.cjs list [seconds]
 *   node out/cast-cli.cjs play <device name or id fragment> <url> [mime] [seconds]
 */
import type { CastDevice, CastMedia, CastSessionState } from '@shared/cast'
import { createCastService } from './service'

const [cmd = 'list', ...rest] = process.argv.slice(2)
const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms))

function describe(d: CastDevice): string {
  const caps = [d.capabilities.audio && 'audio', d.capabilities.video && 'video']
    .filter(Boolean)
    .join('+')
  return `${d.kind.padEnd(10)} ${d.name.padEnd(28)} ${d.address}:${d.port}  ${d.manufacturer} ${d.model}${d.wiim ? ' [WiiM]' : ''}  (${caps || 'unsupported'})  ${d.id}`
}

async function main(): Promise<void> {
  let devices: CastDevice[] = []
  let last: CastSessionState | null = null
  const manager = createCastService({
    onDevicesChanged: (d) => {
      devices = d
    },
    onSessionChanged: (s) => {
      last = s
      console.log(
        `state: ${s.status} ${s.positionSec.toFixed(0)}/${s.durationSec.toFixed(0)}s vol=${s.volume} ${s.error ?? ''}`,
      )
    },
    onEnded: () => console.log('ended'),
    onError: (_id, message) => console.log('error:', message),
  })
  manager.startDiscovery()
  try {
    if (cmd === 'list') {
      await sleep(Number(rest[0] ?? 6) * 1000)
      if (devices.length === 0) console.log('No devices found.')
      for (const d of devices) console.log(describe(d))
      return
    }
    if (cmd === 'play') {
      const [query, url, mime = 'audio/mpeg', secs = '20'] = rest
      if (!query || !url) throw new Error('usage: play <device> <url> [mime] [seconds]')
      const needle = query.toLowerCase()
      let device: CastDevice | undefined
      for (let i = 0; i < 15 && !device; i++) {
        await sleep(1000)
        device = devices.find(
          (d) => d.id.toLowerCase().includes(needle) || d.name.toLowerCase().includes(needle),
        )
      }
      if (!device) throw new Error(`device "${query}" not found`)
      console.log('using', describe(device))
      const media: CastMedia = {
        url,
        mimeType: mime,
        kind: mime.startsWith('video/') ? 'video' : 'audio',
        title: 'Lumen cast test',
        artist: 'Lumen',
        album: 'Manual check',
      }
      await manager.load(device.id, media)
      await sleep(Number(secs) * 1000)
      console.log('final state:', last)
      await manager.stop(device.id)
      return
    }
    throw new Error(`unknown command "${cmd}"`)
  } finally {
    await manager.dispose()
  }
}

main().then(
  () => process.exit(0),
  (err: unknown) => {
    console.error('[cast]', err instanceof Error ? err.message : err)
    process.exit(1)
  },
)
