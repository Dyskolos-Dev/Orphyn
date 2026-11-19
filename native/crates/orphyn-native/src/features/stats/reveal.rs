//! The scroll animations of the stats page: `Reveal` (fade and lift into view, framer-motion
//! `whileInView` with `once`) and `CountUp` (a number counting up the first time it is seen),
//! driven by the native motion helper.
use std::rc::Rc;
use std::time::Duration;

use dioxus::prelude::*;

use crate::i18n::use_i18n;
use crate::motion::{easings, use_tween};
use crate::router::use_router;

/// Share of the viewport kept clear at the bottom before an element counts as in view
/// (`margin: '0px 0px -10% 0px'`).
const BOTTOM_MARGIN: f64 = 0.10;

/// True once the element has entered the viewport (never goes back to false). It re-checks on
/// every scroll of the page until then; `delay` seconds are waited before flipping.
pub fn use_in_view(delay: f64) -> (Signal<bool>, Signal<Option<Rc<MountedData>>>) {
    let router = use_router();
    let window = dioxus_native::use_window();
    let el: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    let mut seen = use_signal(|| false);
    // One pending check at a time.
    let mut checking = use_signal(|| false);
    use_effect(move || {
        let _ = (router.scroll_y)();
        let node = el();
        if *seen.peek() || *checking.peek() {
            return;
        }
        let Some(node) = node else { return };
        checking.set(true);
        let window = window.clone();
        spawn(async move {
            // The rect is 0x0 until the first layout: retry for a few frames.
            for _ in 0..20 {
                if let Ok(r) = node.get_client_rect().await {
                    if r.size.height > 0.0 {
                        let vh =
                            window.surface_size().height as f64 / window.scale_factor().max(0.1);
                        if r.origin.y < vh * (1.0 - BOTTOM_MARGIN) {
                            if delay > 0.0 {
                                tokio::time::sleep(Duration::from_secs_f64(delay)).await;
                            }
                            seen.set(true);
                        }
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            checking.set(false);
        });
    });
    (seen, el)
}

/// Fades and lifts its children in when scrolled into view.
#[component]
pub fn Reveal(
    #[props(default = 0.0)] delay: f64,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    let (seen, mut el) = use_in_view(delay);
    let p = use_tween(if seen() { 1.0 } else { 0.0 }, 0.6, easings::OUT);
    let opacity = p();
    let lift = (1.0 - opacity) * 24.0;
    rsx! {
        div {
            class: "{class}",
            style: "opacity: {opacity}; transform: translateY({lift}px);",
            onmounted: move |e| el.set(Some(e.data())),
            {children}
        }
    }
}

/// Thousands separator of the language (`Intl.NumberFormat`).
pub fn format_number(n: i64, lang: &str) -> String {
    let sep = if lang == "fr" { '\u{202f}' } else { ',' };
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(c);
    }
    if n < 0 {
        out.insert(0, '-');
    }
    out
}

/// Number that counts up the first time it is seen.
#[component]
pub fn CountUp(value: i64) -> Element {
    let t = use_i18n();
    let (seen, mut el) = use_in_view(0.0);
    let p = use_tween(if seen() { 1.0 } else { 0.0 }, 1.4, easings::OUT);
    let shown = (value as f64 * p()).round() as i64;
    rsx! {
        span {
            class: "jrs-num",
            onmounted: move |e| el.set(Some(e.data())),
            {format_number(shown, t.lang().code())}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_number;

    #[test]
    fn groups_thousands_per_language() {
        assert_eq!(format_number(0, "en"), "0");
        assert_eq!(format_number(999, "en"), "999");
        assert_eq!(format_number(1234, "en"), "1,234");
        assert_eq!(format_number(1_234_567, "fr"), "1\u{202f}234\u{202f}567");
        assert_eq!(format_number(-12_345, "en"), "-12,345");
    }
}
