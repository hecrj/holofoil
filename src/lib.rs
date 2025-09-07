pub use bytes::Bytes;
pub use wgpu;

use std::mem;

#[derive(Debug)]
pub struct Pipeline {
    raw: wgpu::RenderPipeline,
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
            back_texture,
        }
    }

    pub fn upload(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        definition: &Structure,
    ) -> Card {
        let instance = device.create_buffer(&wgpu::wgt::BufferDescriptor {
            label: Some("holofoil instance buffer"),
            size: mem::size_of::<Instance>() as u64,
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
            base,
            foil,
            etching,
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
    pub fn prepare(&mut self, queue: &wgpu::Queue, parameters: Parameters) {
        let Parameters { viewport, rotation } = parameters;

        queue.write_buffer(
            &self.instance,
            0,
            bytemuck::cast_slice(&[Instance {
                viewport: [
                    viewport.x as f32,
                    viewport.y as f32,
                    viewport.width as f32,
                    viewport.height as f32,
                ],
                size: [self.width as f32, self.height as f32],
                rotation: [rotation.a.x, rotation.a.y, rotation.a.z, rotation.w],
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parameters {
    pub viewport: Viewport,
    pub rotation: Quaternion,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quaternion {
    pub a: Vector,
    pub w: f32,
}

impl Quaternion {
    pub fn from_radians(a: Vector, angle: f32) -> Self {
        let angle = -angle / 2.0;
        let sin = angle.sin();
        let cos = angle.cos();

        Self { a: a * sin, w: cos }
    }

    pub fn normalize(self) -> Self {
        let d = (self.a.dot(self.a) + self.w * self.w).sqrt();

        Self {
            a: self.a / d,
            w: self.w / d,
        }
    }

    pub fn to_euler(self) -> Vector {
        let pitch = (2.0 * (self.w * self.a.x - self.a.y * self.a.z))
            .clamp(-1.0, 1.0)
            .asin();

        let yaw = (2.0 * (self.w * self.a.y + self.a.z * self.a.x))
            .atan2(1.0 - 2.0 * (self.a.x * self.a.x + self.a.y * self.a.y));

        let roll = (2.0 * (self.w * self.a.z + self.a.x * self.a.y))
            .atan2(1.0 - 2.0 * (self.a.x * self.a.x + self.a.z * self.a.z));

        let normalize = |angle: f32| {
            if angle < 0.0 {
                -angle
            } else {
                (2.0 * std::f32::consts::PI - angle).rem_euclid(2.0 * std::f32::consts::PI)
            }
        };

        Vector {
            x: normalize(pitch),
            y: normalize(yaw),
            z: normalize(roll),
        }
    }
}

impl Default for Quaternion {
    fn default() -> Self {
        Self {
            a: Vector::default(),
            w: 1.0,
        }
    }
}

impl std::ops::Mul for Quaternion {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let a = self.a * rhs.w + rhs.a * self.w + self.a.cross(rhs.a);
        let w = self.w * rhs.w - self.a.dot(rhs.a);

        Self { a, w }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vector {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector {
    pub const X: Self = Self {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };

    pub const Y: Self = Self {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };

    pub const Z: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    pub fn cross(self, rhs: Self) -> Self {
        Self {
            x: self.y * rhs.z - self.z * rhs.y,
            y: self.z * rhs.x - self.x * rhs.z,
            z: self.x * rhs.y - self.y * rhs.x,
        }
    }

    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }
}

impl std::ops::Add for Vector {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl std::ops::Mul<f32> for Vector {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Vector {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
        }
    }
}

impl std::ops::Div<f32> for Vector {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Vector {
            x: self.x / rhs,
            y: self.y / rhs,
            z: self.z / rhs,
        }
    }
}

#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
struct Instance {
    viewport: [f32; 4],
    size: [f32; 2],
    rotation: [f32; 4],
}
