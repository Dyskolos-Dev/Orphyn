//! Tiny forgiving XML helpers: escaping and an element-tree parser. UPnP devices often send
//! sloppy XML, so this tolerates unbalanced tags instead of failing like a strict parser.

use std::collections::HashMap;

pub fn escape_xml(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// Decodes one entity body (the text between `&` and `;`). `None` = not an entity at all;
/// `Some(None)` = a numeric reference out of range (dropped, as the Electron parser did).
fn decode_entity(ent: &str) -> Option<Option<char>> {
    match ent {
        "lt" => Some(Some('<')),
        "gt" => Some(Some('>')),
        "quot" => Some(Some('"')),
        "apos" => Some(Some('\'')),
        "amp" => Some(Some('&')),
        _ => {
            let numeric = ent.strip_prefix('#')?;
            let code = match numeric.strip_prefix('x') {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => numeric.parse::<u32>().ok(),
            };
            Some(code.and_then(char::from_u32))
        }
    }
}

pub fn unescape_xml(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp + 1..];
        let decoded = tail
            .find(';')
            .filter(|&i| i <= 10)
            .and_then(|semi| decode_entity(&tail[..semi]).map(|c| (semi, c)));
        match decoded {
            Some((semi, ch)) => {
                if let Some(c) = ch {
                    out.push(c);
                }
                rest = &tail[semi + 1..];
            }
            None => {
                out.push('&');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

#[derive(Debug, Default, Clone)]
pub struct XmlNode {
    /// Local name (namespace prefix stripped).
    pub name: String,
    #[allow(dead_code)] // only read by tests; part of the tree for completeness
    pub attrs: HashMap<String, String>,
    pub children: Vec<XmlNode>,
    /// Concatenated, unescaped text directly inside this element.
    pub text: String,
}

fn local_name(qname: &str) -> &str {
    qname.rsplit_once(':').map_or(qname, |(_, l)| l)
}

fn parse_attrs(raw: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    let b = raw.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        let key = &raw[start..i];
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] != b'=' {
            continue;
        }
        i += 1;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || (b[i] != b'"' && b[i] != b'\'') {
            continue;
        }
        let q = b[i];
        i += 1;
        let vs = i;
        while i < b.len() && b[i] != q {
            i += 1;
        }
        if !key.is_empty() {
            attrs.insert(local_name(key).to_string(), unescape_xml(&raw[vs..i]));
        }
        i += 1;
    }
    attrs
}

/// Index of the `>` that closes a tag starting at `from`, skipping quoted attribute values.
fn tag_end(s: &str, from: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (i, &c) in s.as_bytes()[from..].iter().enumerate() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == b'"' || c == b'\'' => quote = Some(c),
            None if c == b'>' => return Some(from + i),
            None => {}
        }
    }
    None
}

/// Moves the top open element into its parent.
fn close_top(stack: &mut Vec<XmlNode>) {
    if stack.len() > 1 {
        if let Some(done) = stack.pop() {
            if let Some(parent) = stack.last_mut() {
                parent.children.push(done);
            }
        }
    }
}

/// Parses a document into a tree; the returned synthetic root holds the top-level elements.
pub fn parse_xml(xml: &str) -> XmlNode {
    // Open elements live in `stack` (index 0 is the root); closing one moves it into its parent.
    let mut stack: Vec<XmlNode> = vec![XmlNode {
        name: "#root".into(),
        ..Default::default()
    }];
    let mut pos = 0;
    while pos < xml.len() {
        let rest = &xml[pos..];
        if !rest.starts_with('<') {
            let end = rest.find('<').unwrap_or(rest.len());
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&unescape_xml(&rest[..end]));
            }
            pos += end;
        } else if rest.starts_with("<!--") {
            pos += rest.find("-->").map_or(rest.len(), |i| i + 3);
        } else if let Some(body) = rest.strip_prefix("<![CDATA[") {
            let end = body.find("]]>");
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&body[..end.unwrap_or(body.len())]);
            }
            pos += 9 + end.map_or(body.len(), |i| i + 3);
        } else if rest.starts_with("<?") {
            pos += rest.find("?>").map_or(rest.len(), |i| i + 2);
        } else if rest.starts_with("<!") {
            pos += rest.find('>').map_or(rest.len(), |i| i + 1);
        } else if let Some(close) = rest.strip_prefix("</") {
            let end = close.find('>');
            let name = local_name(close[..end.unwrap_or(close.len())].trim()).to_string();
            // Pop to the matching element (tolerates unbalanced tags).
            if let Some(idx) = (1..stack.len()).rev().find(|&i| stack[i].name == name) {
                while stack.len() > idx {
                    close_top(&mut stack);
                }
            }
            pos += 2 + end.map_or(close.len(), |i| i + 1);
        } else {
            let Some(end) = tag_end(xml, pos + 1) else {
                break;
            };
            let inner = &xml[pos + 1..end];
            let self_closing = inner.ends_with('/');
            let inner = inner.strip_suffix('/').unwrap_or(inner);
            let name_end = inner.find(char::is_whitespace).unwrap_or(inner.len());
            let node = XmlNode {
                name: local_name(&inner[..name_end]).to_string(),
                attrs: parse_attrs(&inner[name_end..]),
                ..Default::default()
            };
            if self_closing {
                if let Some(top) = stack.last_mut() {
                    top.children.push(node);
                }
            } else {
                stack.push(node);
            }
            pos = end + 1;
        }
    }
    while stack.len() > 1 {
        close_top(&mut stack);
    }
    stack.pop().unwrap_or_default()
}

/// First descendant (depth-first) with the given local name.
pub fn find_node<'a>(node: &'a XmlNode, name: &str) -> Option<&'a XmlNode> {
    for child in &node.children {
        if child.name == name {
            return Some(child);
        }
        if let Some(deep) = find_node(child, name) {
            return Some(deep);
        }
    }
    None
}

pub fn find_all<'a>(node: &'a XmlNode, name: &str, out: &mut Vec<&'a XmlNode>) {
    for child in &node.children {
        if child.name == name {
            out.push(child);
        }
        find_all(child, name, out);
    }
}

pub fn text_of(node: &XmlNode, name: &str) -> Option<String> {
    find_node(node, name).map(|n| n.text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_and_unescapes_the_five_entities_and_numeric_references() {
        let raw = "a & b < c > d \" e ' f";
        assert_eq!(escape_xml(raw), "a &amp; b &lt; c &gt; d &quot; e &apos; f");
        assert_eq!(unescape_xml(&escape_xml(raw)), raw);
        assert_eq!(unescape_xml("&#65;&#x42;&amp;lt;"), "AB&lt;");
        assert_eq!(
            unescape_xml("a & b &unknown; &#99999999;"),
            "a & b &unknown; "
        );
    }

    #[test]
    fn parses_nested_elements_attributes_cdata_namespaces_and_self_closing_tags() {
        let root = parse_xml(
            r#"<?xml version="1.0"?><!-- c --><a:root xmlns:a="x" id='1'><a:t>x &amp; y</a:t><e/><d><![CDATA[<raw>]]></d></a:root>"#,
        );
        let r = find_node(&root, "root").unwrap();
        assert_eq!(r.attrs.get("id").map(String::as_str), Some("1"));
        assert_eq!(find_node(r, "t").unwrap().text, "x & y");
        assert!(find_node(r, "e").unwrap().children.is_empty());
        assert_eq!(find_node(r, "d").unwrap().text, "<raw>");
    }

    #[test]
    fn tolerates_unbalanced_and_truncated_markup() {
        let root = parse_xml("<a><b>one<c>two</a><d>");
        let a = find_node(&root, "a").unwrap();
        assert_eq!(find_node(a, "c").unwrap().text, "two");
        assert!(find_node(&root, "d").is_some());
        let _ = parse_xml("<a href='unterminated");
        let _ = parse_xml("<");
        let _ = parse_xml("é<é>é</é>");
    }
}
