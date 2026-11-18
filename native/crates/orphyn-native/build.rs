//! Build-time bundling of the React app's sources, so the native app never carries copies:
//!
//! - `app.css`: the stylesheets listed in `native/styles/manifest.txt`, read from
//!   `src/renderer/src`, `@import`s inlined, with the `html` / `body` / `#root` selectors
//!   re-anchored on the app's root elements (`.jr-root`, `.jr-body`, `.jr-app`): Dioxus renders
//!   inside `<body>`, so the theme attributes live on `.jr-root` instead of `<html>`.
//! - `locales.rs`: every `src/renderer/src/i18n/locales/<lang>/<ns>.json`, as `include_str!`s.
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let native = manifest_dir.join("../..");
    let src = native.join("../src/renderer/src");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    let manifest = native.join("styles/manifest.txt");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let list = fs::read_to_string(&manifest).expect("native/styles/manifest.txt");
    let mut css = String::new();
    for line in list.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        append_css(&src.join(line), &mut css, 0);
    }
    fs::write(out.join("app.css"), rewrite_root_selectors(&css)).unwrap();

    let locales = src.join("i18n/locales");
    println!("cargo:rerun-if-changed={}", locales.display());
    let mut code = String::from("pub static LOCALES: &[(&str, &str, &str)] = &[\n");
    let mut langs: Vec<_> = fs::read_dir(&locales)
        .expect("src/renderer/src/i18n/locales")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    langs.sort();
    for dir in langs {
        let lang = dir.file_name().unwrap().to_string_lossy().to_string();
        let mut files: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        files.sort();
        for f in files {
            println!("cargo:rerun-if-changed={}", f.display());
            let ns = f.file_stem().unwrap().to_string_lossy().to_string();
            let path = fs::canonicalize(&f).unwrap();
            writeln!(
                code,
                "    ({lang:?}, {ns:?}, include_str!({:?})),",
                path.to_string_lossy()
            )
            .unwrap();
        }
    }
    code.push_str("];\n");
    fs::write(out.join("locales.rs"), code).unwrap();
}

fn append_css(path: &Path, out: &mut String, depth: usize) {
    assert!(depth < 16, "@import loop at {}", path.display());
    println!("cargo:rerun-if-changed={}", path.display());
    let text = fs::read_to_string(path).unwrap_or_else(|e| {
        panic!(
            "{}: {e} (listed in native/styles/manifest.txt)",
            path.display()
        )
    });
    let rel = path.to_string_lossy().replace('\\', "/");
    let rel = rel
        .split("src/renderer/src/")
        .last()
        .unwrap_or(&rel)
        .to_string();
    writeln!(out, "/* ==== {rel} ==== */").unwrap();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("@import") {
            let target = rest
                .trim()
                .trim_end_matches(';')
                .trim()
                .trim_start_matches("url(")
                .trim_end_matches(')')
                .trim_matches(|c| c == '"' || c == '\'');
            if target.starts_with('.') {
                append_css(&path.parent().unwrap().join(target), out, depth + 1);
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
}

/// Replaces the `html`, `body` and `#root` selectors (outside declaration blocks and comments).
fn rewrite_root_selectors(css: &str) -> String {
    let bytes = css.as_bytes();
    let mut out = String::with_capacity(css.len() + 1024);
    let mut i = 0;
    // Depth of `{`: selectors appear at depth 0, or 1 inside @media / @supports / @container.
    let mut stack: Vec<bool> = Vec::new(); // true = the block holds rules (an at-rule block)
    let mut prelude_start = 0usize;
    let in_selector = |stack: &Vec<bool>| stack.iter().all(|b| *b);
    while i < bytes.len() {
        if bytes[i..].starts_with(b"/*") {
            let end = css[i + 2..]
                .find("*/")
                .map(|e| i + 2 + e + 2)
                .unwrap_or(bytes.len());
            out.push_str(&css[i..end]);
            i = end;
            continue;
        }
        let c = bytes[i] as char;
        if c == '"' || c == '\'' {
            let end = css[i + 1..]
                .find(c)
                .map(|e| i + 1 + e + 1)
                .unwrap_or(bytes.len());
            out.push_str(&css[i..end]);
            i = end;
            continue;
        }
        match c {
            '{' => {
                let prelude = css[prelude_start..i].trim_start();
                stack.push(prelude.starts_with('@') && !prelude.starts_with("@font-face"));
                out.push(c);
                i += 1;
                prelude_start = i;
                continue;
            }
            '}' => {
                stack.pop();
                out.push(c);
                i += 1;
                prelude_start = i;
                continue;
            }
            ';' => {
                out.push(c);
                i += 1;
                prelude_start = i;
                continue;
            }
            _ => {}
        }
        if in_selector(&stack) {
            let prev = if i == 0 { ' ' } else { bytes[i - 1] as char };
            let boundary_before = prev.is_whitespace()
                || matches!(prev, ',' | '{' | '}' | '(' | '>' | '~' | '+' | ';');
            let replaced = [
                ("html", ".jr-root"),
                ("body", ".jr-body"),
                ("#root", ".jr-app"),
            ]
            .iter()
            .find_map(|(from, to)| {
                let next = bytes.get(i + from.len()).map(|b| *b as char).unwrap_or(' ');
                let boundary_after =
                    next.is_whitespace() || matches!(next, '[' | ':' | ',' | '{' | '.' | ')' | '>');
                (boundary_before && bytes[i..].starts_with(from.as_bytes()) && boundary_after)
                    .then_some((from.len(), *to))
            });
            if let Some((len, to)) = replaced {
                out.push_str(to);
                i += len;
                continue;
            }
        }
        let ch = css[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}
