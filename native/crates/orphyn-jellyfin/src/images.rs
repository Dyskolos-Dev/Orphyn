//! Image URLs (port of `images.ts`): the same source resolution (own tag, then the series / album /
//! parent image), plus device-pixel sizing: screens pass CSS pixels and the window scale factor.
use crate::client::{build_query, Q};
use crate::types::BaseItemDto;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    Primary,
    Backdrop,
    Logo,
    Thumb,
}

impl ImageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ImageKind::Primary => "Primary",
            ImageKind::Backdrop => "Backdrop",
            ImageKind::Logo => "Logo",
            ImageKind::Thumb => "Thumb",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageOptions {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    /// Default 90, like the React app.
    pub quality: Option<u32>,
    pub tag: Option<String>,
}

impl ImageOptions {
    /// A square box of `css_px` CSS pixels at `scale` (the window's device pixel ratio), rounded up
    /// to a 40 px step so nearby sizes share the server's and our image cache.
    pub fn square(css_px: f64, scale: f64, quality: Option<u32>) -> Self {
        let px = device_px(css_px, scale);
        Self {
            max_width: Some(px),
            max_height: Some(px),
            quality,
            tag: None,
        }
    }

    /// A width of `css_px` CSS pixels at `scale`.
    pub fn width(css_px: f64, scale: f64) -> Self {
        Self {
            max_width: Some(device_px(css_px, scale)),
            ..Self::default()
        }
    }
}

/// CSS pixels to device pixels, rounded up to a multiple of 40.
pub fn device_px(css_px: f64, scale: f64) -> u32 {
    let px = (css_px * scale.max(1.0)).ceil() as u32;
    px.div_ceil(40) * 40
}

struct Source {
    id: String,
    tag: String,
    index: Option<u32>,
    kind: Option<ImageKind>,
}

fn own(tag: Option<&String>, id: Option<&String>) -> Option<Source> {
    match (tag, id) {
        (Some(tag), Some(id)) if !tag.is_empty() && !id.is_empty() => Some(Source {
            id: id.clone(),
            tag: tag.clone(),
            index: None,
            kind: None,
        }),
        _ => None,
    }
}

fn resolve(item: &BaseItemDto, kind: ImageKind) -> Option<Source> {
    if item.id.is_empty() {
        return None;
    }
    let id = Some(&item.id);
    match kind {
        ImageKind::Primary => own(item.image_tags.get("Primary"), id)
            .or_else(|| {
                if item.is_type("Episode") || item.is_type("Season") {
                    own(
                        item.series_primary_image_tag.as_ref(),
                        item.series_id.as_ref(),
                    )
                } else {
                    None
                }
            })
            .or_else(|| {
                if item.is_type("Audio") {
                    own(
                        item.album_primary_image_tag.as_ref(),
                        item.album_id.as_ref(),
                    )
                } else {
                    None
                }
            })
            .or_else(|| {
                own(
                    item.parent_primary_image_tag.as_ref(),
                    item.parent_primary_image_item_id.as_ref(),
                )
            }),
        ImageKind::Backdrop => {
            if let Some(tag) = item.backdrop_image_tags.first() {
                return Some(Source {
                    id: item.id.clone(),
                    tag: tag.clone(),
                    index: Some(0),
                    kind: None,
                });
            }
            match (
                item.parent_backdrop_image_tags.first(),
                item.parent_backdrop_item_id.as_ref(),
            ) {
                (Some(tag), Some(pid)) => Some(Source {
                    id: pid.clone(),
                    tag: tag.clone(),
                    index: Some(0),
                    kind: None,
                }),
                _ => None,
            }
        }
        ImageKind::Logo => own(item.image_tags.get("Logo"), id).or_else(|| {
            own(
                item.parent_logo_image_tag.as_ref(),
                item.parent_logo_item_id.as_ref(),
            )
        }),
        ImageKind::Thumb => own(item.image_tags.get("Thumb"), id)
            .or_else(|| {
                own(
                    item.parent_thumb_image_tag.as_ref(),
                    item.parent_thumb_item_id.as_ref(),
                )
            })
            .or_else(|| {
                resolve(item, ImageKind::Primary).map(|s| Source {
                    kind: Some(ImageKind::Primary),
                    ..s
                })
            }),
    }
}

/// The image URL of `item`, or `None` when it has no such image (show the fallback then).
pub fn build_image_url(
    base_url: &str,
    item: &BaseItemDto,
    kind: ImageKind,
    opts: &ImageOptions,
) -> Option<String> {
    let src = resolve(item, kind)?;
    let k = src.kind.unwrap_or(kind).as_str();
    let path = match src.index {
        Some(i) => format!("/Items/{}/Images/{k}/{i}", src.id),
        None => format!("/Items/{}/Images/{k}", src.id),
    };
    let query = build_query(&[
        ("maxWidth", opts.max_width.map(Q::from)),
        ("maxHeight", opts.max_height.map(Q::from)),
        ("quality", Some(Q::from(opts.quality.unwrap_or(90)))),
        ("tag", Some(Q::from(opts.tag.clone().unwrap_or(src.tag)))),
    ]);
    Some(format!("{}{path}?{query}", base_url.trim_end_matches('/')))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(json: serde_json::Value) -> BaseItemDto {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn audio_falls_back_to_its_album_art() {
        let t = item(serde_json::json!({
            "Id": "t1", "Type": "Audio", "AlbumId": "al1", "AlbumPrimaryImageTag": "tagA"
        }));
        assert_eq!(
            build_image_url(
                "https://jf.x/",
                &t,
                ImageKind::Primary,
                &ImageOptions::square(56.0, 2.0, Some(85))
            )
            .as_deref(),
            Some("https://jf.x/Items/al1/Images/Primary?maxWidth=120&maxHeight=120&quality=85&tag=tagA")
        );
    }

    #[test]
    fn backdrop_uses_the_parent_and_an_index() {
        let e = item(serde_json::json!({
            "Id": "e1", "Type": "Episode", "ParentBackdropItemId": "s1",
            "ParentBackdropImageTags": ["b"]
        }));
        assert_eq!(
            build_image_url(
                "https://jf.x",
                &e,
                ImageKind::Backdrop,
                &ImageOptions::default()
            )
            .as_deref(),
            Some("https://jf.x/Items/s1/Images/Backdrop/0?quality=90&tag=b")
        );
        assert_eq!(
            build_image_url(
                "https://jf.x",
                &e,
                ImageKind::Logo,
                &ImageOptions::default()
            ),
            None
        );
    }

    #[test]
    fn device_pixels_round_up_to_40() {
        assert_eq!(device_px(232.0, 1.0), 240);
        assert_eq!(device_px(232.0, 2.0), 480);
        assert_eq!(device_px(40.0, 1.0), 40);
    }
}
