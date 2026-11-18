//! The video surface: a Blitz custom widget (from spike A2). It reads the newest mpv frame, uploads
//! it into a wgpu texture registered with the renderer and paints it over the element's box, so
//! the controls are ordinary RSX/CSS drawn on top.
//!
//! ```ignore
//! let widget = use_hook(|| CustomWidgetAttr::new(VideoWidget::new(controller.mpv()?)));
//! rsx! { object { class: "jr-video__surface", data: widget } }
//! ```
use std::sync::Arc;

use anyrender::{Paint, PaintScene, RenderContext, ResourceId, Scene};
use blitz_dom::node::ComputedStyles;
use blitz_dom::Widget;
use dioxus_native::DeviceHandle;

use super::mpv::Mpv;

pub struct VideoWidget {
    mpv: Arc<Mpv>,
    device: Option<DeviceHandle>,
    texture: Option<(wgpu::Texture, u32, u32)>,
    resource: Option<ResourceId>,
}

impl VideoWidget {
    pub fn new(mpv: Arc<Mpv>) -> Self {
        Self {
            mpv,
            device: None,
            texture: None,
            resource: None,
        }
    }
}

impl Widget for VideoWidget {
    fn can_create_surfaces(&mut self, ctx: &mut dyn RenderContext) {
        self.device = ctx
            .renderer_specific_context()
            .and_then(|c| c.downcast::<DeviceHandle>().ok())
            .map(|b| *b);
    }

    fn destroy_surfaces(&mut self) {
        self.texture = None;
        self.resource = None;
        self.device = None;
    }

    fn requires_redraw(&self) -> bool {
        true
    }

    fn paint(
        &mut self,
        ctx: &mut dyn RenderContext,
        _styles: &ComputedStyles,
        width: u32,
        height: u32,
        _scale: f64,
    ) -> Scene {
        let mut scene = Scene::new();
        let Some(dev) = &self.device else {
            return scene;
        };
        // Ask mpv for frames at the box's pixel size: the texture then maps 1:1.
        self.mpv
            .target
            .0
            .store(width, std::sync::atomic::Ordering::Relaxed);
        self.mpv
            .target
            .1
            .store(height, std::sync::atomic::Ordering::Relaxed);
        {
            let mut f = self.mpv.frame.lock().unwrap_or_else(|p| p.into_inner());
            if f.fresh && f.w > 0 {
                let (w, h) = (f.w, f.h);
                if self.texture.as_ref().map(|t| (t.1, t.2)) != Some((w, h)) {
                    if let Some(id) = self.resource.take() {
                        ctx.unregister_resource(id);
                    }
                    let tex = dev.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("mpv-frame"),
                        size: wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    self.resource = ctx.try_register_custom_resource(Box::new(tex.clone())).ok();
                    self.texture = Some((tex, w, h));
                }
                if let Some((tex, _, _)) = &self.texture {
                    dev.queue.write_texture(
                        tex.as_image_copy(),
                        &f.data,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(w * 4),
                            rows_per_image: Some(h),
                        },
                        wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                f.fresh = false;
            }
        }
        if let Some(id) = self.resource {
            let brush = Paint::Resource(peniko::ImageBrush {
                image: id,
                sampler: Default::default(),
            });
            scene.fill(
                peniko::Fill::NonZero,
                kurbo::Affine::IDENTITY,
                &brush,
                None,
                &kurbo::Rect::new(0.0, 0.0, width as f64, height as f64),
            );
        }
        scene
    }
}
