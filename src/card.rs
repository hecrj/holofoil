use crate::{Bytes, Quaternion};

#[derive(Debug, Clone)]
pub struct Card {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) instance: wgpu::Buffer,
    pub(crate) _base: wgpu::Texture,
    pub(crate) _foil: Option<wgpu::Texture>,
    pub(crate) _etching: Option<wgpu::Texture>,
    pub(crate) binding: wgpu::BindGroup,
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
    pub base: Image,
    pub foil: Option<Mask>,
    pub etching: Option<Mask>,
    pub width: u32,
}

#[derive(Debug, Clone)]
pub struct Image {
    pub rgba: Bytes,
    pub size: u32,
}

impl Image {
    pub(crate) fn upload(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
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
    pub(crate) fn upload(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
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

#[derive(Debug, Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(C)]
pub struct Instance {
    viewport: [f32; 4],
    size: [f32; 2],
    rotation: [f32; 4],
}
