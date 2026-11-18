//! Bundled assets: the stylesheet built by `build.rs`, the Blitz overrides, the logo (the React
//! app's own file) and the Inter font (`@fontsource-variable/inter`, latin subset, OFL).
use std::sync::{Arc, OnceLock};

use base64::Engine;
use dioxus_native::FontContext;

/// The React app's stylesheets, bundled by `build.rs` (see `native/styles/manifest.txt`).
pub const APP_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/app.css"));
/// Blitz-only fixes (`native/styles/overrides.css`, then one file per feature port in
/// `native/styles/overrides/` so parallel ports never edit the same file).
pub const OVERRIDES_CSS: &str = concat!(
    include_str!("../../../styles/overrides.css"),
    include_str!("../../../styles/overrides/music-library.css"),
    include_str!("../../../styles/overrides/music-panels.css"),
    include_str!("../../../styles/overrides/video-browse.css"),
    include_str!("../../../styles/overrides/video-player.css"),
    include_str!("../../../styles/overrides/settings.css"),
    include_str!("../../../styles/overrides/search.css"),
);

const LOGO_PNG: &[u8] = include_bytes!("../../../../src/renderer/src/assets/orphyn-logo.png");
const INTER: &[u8] = include_bytes!("../../../assets/fonts/inter-latin-wght-normal.woff2");
const INTER_ITALIC: &[u8] = include_bytes!("../../../assets/fonts/inter-latin-wght-italic.woff2");

/// `BrandLogo`'s image, as a data URL.
pub fn logo_url() -> String {
    static URL: OnceLock<String> = OnceLock::new();
    URL.get_or_init(|| {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(LOGO_PNG)
        )
    })
    .clone()
}

/// System fonts plus Inter registered as `Inter Variable`, the family `tokens.css` asks for.
pub fn font_context() -> FontContext {
    use parley::fontique::{Blob, Collection, CollectionOptions, FontInfoOverride, SourceCache};
    let mut ctx = FontContext {
        source_cache: SourceCache::new_shared(),
        collection: Collection::new(CollectionOptions {
            shared: false,
            system_fonts: true,
        }),
    };
    for data in [INTER, INTER_ITALIC] {
        let decoded = blitz_dom::util::decode_font_bytes(data).into_owned();
        ctx.collection.register_fonts(
            Blob::new(Arc::new(decoded) as _),
            Some(FontInfoOverride {
                family_name: Some("Inter Variable"),
                ..FontInfoOverride::default()
            }),
        );
    }
    ctx
}
