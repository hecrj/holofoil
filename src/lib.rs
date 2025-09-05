pub use bytes::Bytes;
pub use wgpu;

use std::mem;

#[derive(Debug)]
pub struct Pipeline {
    raw: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    uniforms_binding: wgpu::BindGroup,
    textures_layout: wgpu::BindGroupLayout,
    back_texture: wgpu::Texture,
}

macro_rules! load_wgsl {
    ($path:literal) => {
        if cfg!(all(not(target_arch = "wasm32"), debug_assertions)) {
            $crate::wgpu::ShaderModuleDescriptor {
                label: Some($path),
                source: $crate::wgpu::ShaderSource::Wgsl(
                    ::std::fs::read_to_string(format!(
                        "{}/src/{}",
                        env!("CARGO_MANIFEST_DIR"),
                        $path
                    ))
                    .unwrap()
                    .into(),
                ),
            }
        } else {
            $crate::wgpu::include_wgsl!($path)
        }
    };
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        back_texture: Layer,
    ) -> Self {
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("holofoil uniforms buffer"),
            size: mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("holofoil sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });

        let back_texture = back_texture.upload(device, queue);

        let uniforms_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("holofoil uniforms layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let uniforms_binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &uniforms_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniforms,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &back_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
            ],
        });

        let textures_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("holofoil texture layout"),
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("holofoil pipeline layout"),
            bind_group_layouts: &[&uniforms_layout, &textures_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(load_wgsl!("./shader.wgsl"));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("holofoil pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array!(
                        // Position
                        0 => Float32x2,
                        // Size
                        1 => Uint32x2,
                        // Rotation
                        2 => Float32,
                    ),
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            raw: pipeline,
            uniforms,
            uniforms_binding,
            textures_layout,
            back_texture,
        }
    }

    pub fn upload(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        definition: Structure,
    ) -> Result<Card, wgpu::Error> {
        let instance = device.create_buffer(&wgpu::wgt::BufferDescriptor {
            label: Some("holofoil instance buffer"),
            size: mem::size_of::<Instance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let base = definition.base.upload(device, queue);
        let foil = definition.foil.map(|foil| foil.upload(device, queue));
        let etching = definition
            .etching
            .map(|etching| etching.upload(device, queue));

        let base_view = base.create_view(&wgpu::TextureViewDescriptor::default());

        let foil_view = foil
            .as_ref()
            .map(|foil| foil.create_view(&wgpu::TextureViewDescriptor::default()));

        let etching_view = etching
            .as_ref()
            .map(|etching| etching.create_view(&wgpu::TextureViewDescriptor::default()));

        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.textures_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&base_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &foil_view.unwrap_or(base_view.clone()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(
                        &etching_view.unwrap_or(base_view.clone()),
                    ),
                },
            ],
        });

        Ok(Card {
            instance,
            base,
            foil,
            etching,
            binding,
            width: definition.width,
            height: definition.base.size,
        })
    }

    pub fn render(
        &self,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'_>,
        resolution: (u32, u32),
        card: &Card,
    ) {
        queue.write_buffer(
            &self.uniforms,
            0,
            bytemuck::cast_slice(&[Uniforms {
                resolution: [resolution.0, resolution.1],
                _padding: [0, 0],
            }]),
        );

        render_pass.set_pipeline(&self.raw);
        render_pass.set_bind_group(0, &self.uniforms_binding, &[]);
        render_pass.set_bind_group(1, &card.binding, &[]);
        render_pass.set_vertex_buffer(0, card.instance.slice(..));
        render_pass.draw(0..6, 0..1);
    }
}

#[derive(Debug, Clone)]
pub struct Card {
    width: u32,
    height: u32,
    instance: wgpu::Buffer,
    base: wgpu::Texture,
    foil: Option<wgpu::Texture>,
    etching: Option<wgpu::Texture>,
    binding: wgpu::BindGroup,
}

impl Card {
    pub fn prepare(&mut self, queue: &wgpu::Queue, x: f32, y: f32, rotation: f32) {
        queue.write_buffer(
            &self.instance,
            0,
            bytemuck::cast_slice(&[Instance {
                position: [x, y],
                size: [self.width, self.height],
                rotation,
            }]),
        );
    }
}

#[derive(Debug, Clone)]
pub struct Structure {
    pub base: Layer,
    pub foil: Option<Mask>,
    pub etching: Option<Mask>,
    pub width: u32,
}

#[derive(Debug, Clone)]
pub struct Layer {
    pub rgba: Bytes,
    pub size: u32,
}

impl Layer {
    fn upload(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
        use wgpu::util::DeviceExt;

        device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: self.size,
                    height: self.size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &self.rgba,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Mask {
    pub pixels: Bytes,
    pub size: u32,
}

impl Mask {
    fn upload(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
        use wgpu::util::DeviceExt;

        device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: self.size,
                    height: self.size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &self.pixels,
        )
    }
}

#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
struct Instance {
    position: [f32; 2],
    size: [u32; 2],
    rotation: f32,
}

#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
struct Uniforms {
    resolution: [u32; 2],
    _padding: [u32; 2],
}
