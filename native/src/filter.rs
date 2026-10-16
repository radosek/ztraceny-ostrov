//! Optional picture filters drawn with the game frame as a texture: smooth (bilinear) and CRT
//! (scanlines, shadow mask, slight vignette). Sharp pixels use pixels' own renderer.

use pixels::{PixelsContext, wgpu};

const SHADER: &str = r#"
struct U { mode: u32, out_h: f32, _a: f32, _b: f32 };
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;
@group(0) @binding(2) var<uniform> u: U;

struct V { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs(@builtin(vertex_index) i: u32) -> V {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var o: V;
    o.pos = vec4(p[i], 0.0, 1.0);
    o.uv = vec2((p[i].x + 1.0) * 0.5, 1.0 - (p[i].y + 1.0) * 0.5);
    return o;
}

@fragment
fn fs(v: V) -> @location(0) vec4<f32> {
    var c = textureSample(tex, smp, v.uv).rgb;
    if (u.mode == 2u) {
        // scanline: darken between source rows
        let row = fract(v.uv.y * 480.0);
        let scan = 0.65 + 0.35 * sin(row * 3.14159);
        // aperture grille every output pixel triplet
        let px = u32(v.pos.x) % 3u;
        var mask = vec3(0.85, 0.85, 0.85);
        if (px == 0u) { mask.r = 1.0; } else if (px == 1u) { mask.g = 1.0; } else { mask.b = 1.0; }
        let d = v.uv - vec2(0.5, 0.5);
        let vig = 1.0 - dot(d, d) * 0.6;
        c = c * scan * mask * vig * 1.15;
    }
    return vec4(c, 1.0);
}
"#;

pub struct Filter {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    linear: wgpu::Sampler,
    uniform: wgpu::Buffer,
}

impl Filter {
    pub fn new(ctx: &PixelsContext, target: wgpu::TextureFormat) -> Self {
        let d = &ctx.device;
        let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ostrov_filter"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ostrov_filter"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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
        let pl = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ostrov_filter"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let pipeline = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ostrov_filter"),
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState { format: target, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let linear = d.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ostrov_filter"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { pipeline, layout, linear, uniform }
    }

    /// Draw the frame texture into the letterboxed area `clip` with `mode` 1 = smooth, 2 = CRT.
    pub fn render(&self, enc: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, ctx: &PixelsContext, clip: (u32, u32, u32, u32), mode: u32) {
        let mut u = [0u8; 16];
        u[..4].copy_from_slice(&mode.to_le_bytes());
        u[4..8].copy_from_slice(&(clip.3 as f32).to_le_bytes());
        ctx.queue.write_buffer(&self.uniform, 0, &u);
        let view = ctx.texture.create_view(&Default::default());
        let bind = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ostrov_filter"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.linear) },
                wgpu::BindGroupEntry { binding: 2, resource: self.uniform.as_entire_binding() },
            ],
        });
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ostrov_filter"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.set_scissor_rect(clip.0, clip.1, clip.2, clip.3);
        pass.set_viewport(clip.0 as f32, clip.1 as f32, clip.2 as f32, clip.3 as f32, 0.0, 1.0);
        pass.draw(0..3, 0..1);
    }
}
