/** Tiny XML helpers (no parser dependency): escaping and a forgiving element-tree parser. */

export function escapeXml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;')
}

export function unescapeXml(value: string): string {
  return value.replace(/&(#x[0-9a-fA-F]+|#\d+|lt|gt|quot|apos|amp);/g, (_m, ent: string) => {
    switch (ent) {
      case 'lt':
        return '<'
      case 'gt':
        return '>'
      case 'quot':
        return '"'
      case 'apos':
        return "'"
      case 'amp':
        return '&'
      default: {
        const code = ent.startsWith('#x') ? parseInt(ent.slice(2), 16) : parseInt(ent.slice(1), 10)
        return Number.isFinite(code) && code <= 0x10ffff ? String.fromCodePoint(code) : ''
      }
    }
  })
}

export interface XmlNode {
  /** Local name (namespace prefix stripped). */
  name: string
  attrs: Record<string, string>
  children: XmlNode[]
  /** Concatenated, unescaped text content directly inside this element. */
  text: string
}

const TOKEN =
  /<!--[\s\S]*?-->|<!\[CDATA\[([\s\S]*?)\]\]>|<\?[\s\S]*?\?>|<!DOCTYPE[^>]*>|<(\/?)([^\s/>]+)((?:\s+[^\s=/>]+\s*=\s*(?:"[^"]*"|'[^']*'))*)\s*(\/?)>|([^<]+)/g
const ATTR = /([^\s=/>]+)\s*=\s*(?:"([^"]*)"|'([^']*)')/g

function localName(qname: string): string {
  const i = qname.indexOf(':')
  return i >= 0 ? qname.slice(i + 1) : qname
}

/** Parses an XML document into a tree; returns a synthetic root whose children are the top elements. */
export function parseXml(xml: string): XmlNode {
  const root: XmlNode = { name: '#root', attrs: {}, children: [], text: '' }
  const stack: XmlNode[] = [root]
  const tokens = new RegExp(TOKEN.source, 'g')
  let m: RegExpExecArray | null
  while ((m = tokens.exec(xml)) !== null) {
    const top = stack[stack.length - 1] as XmlNode
    if (m[1] !== undefined) {
      top.text += m[1]
    } else if (m[3] !== undefined) {
      if (m[2] === '/') {
        const name = localName(m[3])
        // Pop to the matching element (tolerates unbalanced tags).
        for (let i = stack.length - 1; i > 0; i--) {
          if ((stack[i] as XmlNode).name === name) {
            stack.length = i
            break
          }
        }
      } else {
        const attrs: Record<string, string> = {}
        const attrRe = new RegExp(ATTR.source, 'g')
        let a: RegExpExecArray | null
        const raw = m[4] ?? ''
        while ((a = attrRe.exec(raw)) !== null) {
          attrs[localName(a[1] as string)] = unescapeXml(a[2] ?? a[3] ?? '')
        }
        const node: XmlNode = { name: localName(m[3]), attrs, children: [], text: '' }
        top.children.push(node)
        if (m[5] !== '/') stack.push(node)
      }
    } else if (m[6] !== undefined) {
      top.text += unescapeXml(m[6])
    }
  }
  return root
}

/** First descendant (depth-first) with the given local name. */
export function findNode(node: XmlNode, name: string): XmlNode | undefined {
  for (const child of node.children) {
    if (child.name === name) return child
    const deep = findNode(child, name)
    if (deep) return deep
  }
  return undefined
}

export function findAll(node: XmlNode, name: string, out: XmlNode[] = []): XmlNode[] {
  for (const child of node.children) {
    if (child.name === name) out.push(child)
    findAll(child, name, out)
  }
  return out
}

export function textOf(node: XmlNode, name: string): string | undefined {
  const found = findNode(node, name)
  return found ? found.text.trim() : undefined
}
