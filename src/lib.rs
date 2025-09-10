pub use bytes::Bytes;
pub use wgpu;

mod quaternion;
mod vector;

pub mod card;

pub use card::Card;
pub use quaternion::Quaternion;
pub use vector::Vector;

use std::mem;

#[derive(Debug)]
pub struct Pipeline {
    raw: wgpu::RenderPipeline,
    uniforms_binding: wgpu::BindGroup,
    textures_layout: wgpu::BindGroupLayout,
    configuration: (wgpu::Buffer, Configuration),
    _back_texture: wgpu::Texture,
}

macro_rules! load_wgsl {
    ($path:literal, $format:expr) => {
        $crate::wgpu::ShaderModuleDescriptor {
            label: Some($path),
            source: $crate::wgpu::ShaderSource::Wgsl(
                if cfg!(all(not(target_arch = "wasm32"), debug_assertions)) {
                    let mut shader = ::std::fs::read_to_string(format!(
                        "{}/src/{}",
                        env!("CARGO_MANIFEST_DIR"),
                        $path
                    ))
                    .unwrap();

                    shader.push_str("\n");

                    shader.push_str(if $format.is_srgb() {
                        include_str!("./shader/linear_rgb.wgsl")
                    } else {
                        include_str!("./shader/srgb.wgsl")
                    });

                    shader.into()
                } else if $format.is_srgb() {
                    concat!(
                        include_str!($path),
                        "\n",
                        include_str!("./shader/linear_rgb.wgsl")
                    )
                    .into()
                } else {
                    concat!(
                        include_str!($path),
                        "\n",
                        include_str!("./shader/srgb.wgsl")
                    )
                    .into()
                },
            ),
        }
    };
}

impl Pipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        back_texture: card::Image,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("holofoil sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });

        let configuration = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("holofoil parameters"),
            size: mem::size_of::<Parameters>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        queue.write_buffer(
            &configuration,
            0,
            bytemuck::cast_slice(&[Parameters::from(Configuration::default())]),
        );

        let back_texture = back_texture.upload(device, queue);

        let uniforms_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("holofoil uniforms layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
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
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &back_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: configuration.as_entire_binding(),
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

        let shader = device.create_shader_module(load_wgsl!("./shader.wgsl", format));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("holofoil pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: mem::size_of::<card::Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array!(
                        // Viewport
                        0 => Float32x4,
                        // Size
                        1 => Float32x2,
                        // Rotation
                        2 => Float32x4,
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
            uniforms_binding,
            textures_layout,
            configuration: (configuration, Configuration::default()),
            _back_texture: back_texture,
        }
    }

    pub fn configure(&self, queue: &wgpu::Queue, configuration: Configuration) {
        let (buffer, last) = &self.configuration;

        if *last == configuration {
            return;
        }

        queue.write_buffer(
            buffer,
            0,
            bytemuck::cast_slice(&[Parameters::from(configuration)]),
        );
    }

    pub fn upload(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        definition: &card::Structure,
    ) -> Card {
        let instance = device.create_buffer(&wgpu::wgt::BufferDescriptor {
            label: Some("holofoil instance buffer"),
            size: mem::size_of::<card::Instance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let base = definition.base.upload(device, queue);
        let foil = definition
            .foil
            .as_ref()
            .map(|foil| foil.upload(device, queue));
        let etching = definition
            .etching
            .as_ref()
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

        Card {
            instance,
            _base: base,
            _foil: foil,
            _etching: etching,
            binding,
            width: definition.width,
            height: definition.base.size,
        }
    }

    pub fn render(&self, render_pass: &mut wgpu::RenderPass<'_>, card: &Card) {
        render_pass.set_pipeline(&self.raw);
        render_pass.set_bind_group(0, &self.uniforms_binding, &[]);
        render_pass.set_bind_group(1, &card.binding, &[]);
        render_pass.set_vertex_buffer(0, card.instance.slice(..));
        render_pass.draw(0..6, 0..1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Configuration {
    pub n_samples: u32,
    pub max_iterations: u32,
    pub light: Light,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            n_samples: 2,
            max_iterations: 128,
            light: Light {
                position: Vector {
                    x: 3.0,
                    y: 4.0,
                    z: -20.0,
                },
                power: 400.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Light {
    pub position: Vector,
    pub power: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
pub struct Parameters {
    n_samples: u32,
    max_iterations: u32,
    _padding: [u32; 2],
    light_position: [f32; 3],
    light_power: f32,
}

impl From<Configuration> for Parameters {
    fn from(configuration: Configuration) -> Self {
        Self {
            n_samples: configuration.n_samples,
            max_iterations: configuration.max_iterations,
            light_position: [
                configuration.light.position.x,
                configuration.light.position.y,
                configuration.light.position.z,
            ],
            light_power: configuration.light.power,
            _padding: [0, 0],
        }
    }
}
