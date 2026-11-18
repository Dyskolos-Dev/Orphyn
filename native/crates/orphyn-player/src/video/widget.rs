//! The video surface: a Blitz custom widget (from spike A2). It reads the newest mpv frame, puts
//! it in a wgpu texture registered with the renderer and paints it over the element's box, so the
//! controls are ordinary RSX/CSS drawn on top.
//!
//! YUV pipeline: the packed 4:2:2 frame is uploaded as is (half the bytes of BGRA), a small render
//! pass converts it to RGB into the registered texture, and the scene scales that texture into
//! the box (`object-fit: contain`), so the GPU does both the conversion and the scaling.
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

use super::mpv::{Frame, Mpv, Pipeline, PixelFormat};

const SHADER: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
// x: mode (0 = bgrx, 1 = yuyv), y: flags (1 = BT.709, else BT.601), z: frame width.
@group(0) @binding(1) var<uniform> params: vec4<u32>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    return vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let px = vec2<i32>(pos.xy);
    if (params.x == 0u) {
        return vec4<f32>(textureLoad(src, px, 0).rgb, 1.0);
    }
    // One texel = two pixels: r = Y0, g = U, b = Y1, a = V.
    let t = textureLoad(src, vec2<i32>(px.x / 2, px.y), 0);
    let odd = (px.x & 1) == 1;
    var y = select(t.r, t.b, odd);
    var uv = vec2<f32>(t.g, t.a);
    if (odd) {
        // Chroma sits on the even pixels: the odd one takes the mean with the next pair.
        let last = i32(params.z) / 2 - 1;
        let n = textureLoad(src, vec2<i32>(min(px.x / 2 + 1, last), px.y), 0);
        uv = (uv + vec2<f32>(n.g, n.a)) * 0.5;
    }
    // Limited range (16-235 luma, 16-240 chroma).
    y = (y - 16.0 / 255.0) * (255.0 / 219.0);
    let c = (uv - vec2<f32>(128.0 / 255.0)) * (255.0 / 224.0);
    var rgb: vec3<f32>;
    if ((params.y & 1u) == 1u) {
        rgb = vec3<f32>(y + 1.5748 * c.y, y - 0.1873 * c.x - 0.4681 * c.y, y + 1.8556 * c.x);
    } else {
        rgb = vec3<f32>(y + 1.402 * c.y, y - 0.344136 * c.x - 0.714136 * c.y, y + 1.772 * c.x);
    }
    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
"#;

/// The YUV → RGB pass: its pipeline and the textures of the current frame size.
struct Converter {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    params: wgpu::Buffer,
    /// (source texture, bind group, format, w, h)
    source: Option<(wgpu::Texture, wgpu::BindGroup, PixelFormat, u32, u32)>,
}

impl Converter {
    fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("video-yuv"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("video-yuv"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("video-yuv"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("video-yuv"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: OUTPUT_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("video-yuv-params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            layout,
            params,
            source: None,
        }
    }

    /// Uploads `f` and converts it into `out` (same pixel size as the frame).
    fn convert(&mut self, dev: &DeviceHandle, f: &Frame, out: &wgpu::Texture) {
        let (tex_w, tex_format) = match f.format {
            PixelFormat::Yuyv => (f.w / 2, wgpu::TextureFormat::Rgba8Unorm),
            PixelFormat::Bgrx => (f.w, wgpu::TextureFormat::Bgra8Unorm),
        };
        if self
            .source
            .as_ref()
            .map(|s| (s.2, s.3, s.4))
            != Some((f.format, f.w, f.h))
        {
            let tex = dev.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("mpv-frame-src"),
                size: extent(tex_w, f.h),
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: tex_format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            let bind = dev.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("video-yuv"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.params.as_entire_binding(),
                    },
                ],
            });
            self.source = Some((tex, bind, f.format, f.w, f.h));
        }
        let Some((src, bind, ..)) = &self.source else {
            return;
        };
        let mode = u32::from(f.format == PixelFormat::Yuyv);
        let params = [mode, u32::from(f.bt709), f.w, 0];
        let bytes: Vec<u8> = params.iter().flat_map(|v| v.to_le_bytes()).collect();
        dev.queue.write_buffer(&self.params, 0, &bytes);
        dev.queue.write_texture(
            src.as_image_copy(),
            &f.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(f.format.bytes_per_row(f.w)),
                rows_per_image: Some(f.h),
            },
            extent(tex_w, f.h),
        );
        let view = out.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = dev
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("video-yuv"),
            });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("video-yuv"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        }
        dev.queue.submit([enc.finish()]);
    }
}

/// Format of the texture the scene samples (the one the spike proved with this renderer).
const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

fn extent(width: u32, height: u32) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    }
}

/// Where a `w`×`h` frame goes in a `bw`×`bh` box with `object-fit: contain`: (scale, x, y).
pub fn contain(w: u32, h: u32, bw: u32, bh: u32) -> (f64, f64, f64) {
    if w == 0 || h == 0 {
        return (1.0, 0.0, 0.0);
    }
    let s = (bw as f64 / w as f64).min(bh as f64 / h as f64);
    (
        s,
        (bw as f64 - w as f64 * s) / 2.0,
        (bh as f64 - h as f64 * s) / 2.0,
    )
}

pub struct VideoWidget {
    mpv: Arc<Mpv>,
    device: Option<DeviceHandle>,
    converter: Option<Converter>,
    /// The texture the scene paints: (texture, w, h).
    texture: Option<(wgpu::Texture, u32, u32)>,
    resource: Option<ResourceId>,
}

impl VideoWidget {
    pub fn new(mpv: Arc<Mpv>) -> Self {
        Self {
            mpv,
            device: None,
            converter: None,
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
        self.converter = None;
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
        let Some(dev) = self.device.clone() else {
            return scene;
        };
        let legacy = self.mpv.pipeline == Pipeline::Bgra;
        // The BGRA pipeline renders at the box's pixel size; the YUV one ignores it.
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
                    let usage = if legacy {
                        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
                    } else {
                        wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::RENDER_ATTACHMENT
                    };
                    let tex = dev.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("mpv-frame"),
                        size: extent(w, h),
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: OUTPUT_FORMAT,
                        usage,
                        view_formats: &[],
                    });
                    self.resource = ctx.try_register_custom_resource(Box::new(tex.clone())).ok();
                    self.texture = Some((tex, w, h));
                }
                if let Some((tex, _, _)) = &self.texture {
                    if legacy {
                        dev.queue.write_texture(
                            tex.as_image_copy(),
                            &f.data,
                            wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(w * 4),
                                rows_per_image: Some(h),
                            },
                            extent(w, h),
                        );
                    } else {
                        self.converter
                            .get_or_insert_with(|| Converter::new(&dev.device))
                            .convert(&dev, &f, tex);
                    }
                }
                f.fresh = false;
            }
        }
        if let (Some(id), Some((_, w, h))) = (self.resource, &self.texture) {
            let brush = Paint::Resource(peniko::ImageBrush {
                image: id,
                sampler: Default::default(),
            });
            let (s, x, y) = contain(*w, *h, width, height);
            scene.fill(
                peniko::Fill::NonZero,
                kurbo::Affine::translate((x, y)) * kurbo::Affine::scale(s),
                &brush,
                None,
                &kurbo::Rect::new(0.0, 0.0, *w as f64, *h as f64),
            );
        }
        scene
    }
}

#[cfg(test)]
mod tests {
    use super::contain;

    #[test]
    fn contain_letterboxes_and_pillarboxes() {
        assert_eq!(contain(1920, 1080, 2560, 1440), (4.0 / 3.0, 0.0, 0.0));
        let (s, x, y) = contain(1920, 800, 1920, 1080);
        assert_eq!((s, x), (1.0, 0.0));
        assert_eq!(y, 140.0);
        let (s, x, y) = contain(1440, 1080, 1920, 1080);
        assert_eq!((s, x, y), (1.0, 240.0, 0.0));
        assert_eq!(contain(0, 0, 10, 10), (1.0, 0.0, 0.0));
    }
}
