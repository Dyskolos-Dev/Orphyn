// Generates assets/icons/*.svg from the lucide-react icon data the app uses (run from repo root).
import { writeFileSync } from 'node:fs'
const names = ['play','pause','shuffle','heart','ellipsis','skip-back','skip-forward','repeat','list-music','volume-2','message-square-quote','music','house','disc-3','mic-vocal','users','list-plus','chevron-down','film','search','radio','tv','clock','maximize','x','chevron-left','chevron-right']
for (const n of names) {
  let m
  try { m = await import(new URL(`../../node_modules/lucide-react/dist/esm/icons/${n}.mjs`, import.meta.url)) } catch { console.log('missing', n); continue }
  if (!m.__iconData) { console.log('alias', n); continue }
  const body = m.__iconData.node.map(([t, a]) => `<${t} ${Object.entries(a).filter(([k]) => k !== 'key').map(([k, v]) => `${k}="${v}"`).join(' ')}/>`).join('')
  writeFileSync(new URL(`./assets/icons/${n}.svg`, import.meta.url), `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`)
}
