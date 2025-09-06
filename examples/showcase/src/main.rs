use holofoil::{Bytes, Card, Layer, Mask, Pipeline, Structure};

use iced::mouse;
use iced::theme;
use iced::time::{Duration, Instant};
use iced::wgpu;
use iced::widget::shader;
use iced::window;
use iced::{Color, Element, Fill, Rectangle, Size, Subscription, Task, Theme};

use std::sync::{Arc, Mutex};

fn main() -> iced::Result {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    iced::application::timed(
        Showcase::new,
        Showcase::update,
        Showcase::subscription,
        Showcase::view,
    )
    .theme(|_| Theme::CatppuccinMocha)
    .style(|_, theme| theme::Style {
        background_color: Color::BLACK,
        ..theme::Base::base(theme)
    })
    .run()
}

#[derive(Debug)]
struct Showcase {
    viewer: Viewer,
}

#[derive(Debug)]
enum Message {
    Booted,
    FrameRequested,
}

impl Showcase {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                viewer: Viewer {
                    card: Arc::new(umbreon()),
                    cache: Arc::new(Mutex::new(Cache::new())),
                    started_at: Instant::now(),
                    now: Instant::now(),
                },
            },
            Task::done(Message::Booted), // TODO: Provide `Instant` to `new`
        )
    }

    fn update(&mut self, message: Message, now: Instant) {
        match message {
            Message::Booted => {
                self.viewer.started_at = now;
                self.viewer.now = now;
            }
            Message::FrameRequested => {
                self.viewer.now = now;
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        window::frames().map(|_| Message::FrameRequested)
    }

    fn view(&self) -> Element<'_, Message> {
        shader(&self.viewer).width(Fill).height(Fill).into()
    }
}

#[derive(Debug)]
struct Viewer {
    card: Arc<Structure>,
    cache: Arc<Mutex<Cache>>,
    started_at: Instant,
    now: Instant,
}

impl shader::Program<Message> for Viewer {
    type State = ();
    type Primitive = Holofoil;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        Holofoil {
            card: self.card.clone(),
            cache: self.cache.clone(),
            delta: self.now.duration_since(self.started_at),
        }
    }
}

#[derive(Debug)]
struct Holofoil {
    card: Arc<Structure>,
    cache: Arc<Mutex<Cache>>,
    delta: Duration,
}

impl shader::Primitive for Holofoil {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        storage: &mut shader::Storage,
        bounds: &Rectangle,
        viewport: &shader::Viewport,
    ) {
        if !storage.has::<Pipeline>() {
            storage.store(Pipeline::new(
                device,
                queue,
                format,
                load_image(include_bytes!("../assets/pokemon_tcg_back.png")),
            ));
        }

        let pipeline = storage.get::<Pipeline>().unwrap();
        let mut cache = self.cache.lock().unwrap();

        let mut card = cache
            .card
            .take()
            .unwrap_or_else(|| pipeline.upload(device, queue, &self.card));

        const ROTATION_SPEED: f32 = std::f32::consts::PI / 4.0;

        card.prepare(queue, 0.0, 0.0, ROTATION_SPEED * self.delta.as_secs_f32());
        cache.card = Some(card);

        if let Some(bounds) = (*bounds * viewport.scale_factor()).snap() {
            cache.resolution = Size::new(bounds.width, bounds.height);
            cache.queue = Some(queue.clone());
        }
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        storage: &shader::Storage,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let pipeline = storage.get::<Pipeline>().unwrap();
        let cache = self.cache.lock().unwrap();

        let Some(card) = &cache.card else {
            return;
        };

        let Some(queue) = &cache.queue else {
            return;
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("holofoil shader widget"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_scissor_rect(
            clip_bounds.x,
            clip_bounds.y,
            clip_bounds.width,
            clip_bounds.height,
        );

        pipeline.render(
            queue,
            &mut render_pass,
            (cache.resolution.width, cache.resolution.height),
            card,
        );
    }
}

#[derive(Debug)]
struct Cache {
    card: Option<Card>,
    resolution: Size<u32>,
    queue: Option<wgpu::Queue>, // No one can stop me
}

impl Cache {
    fn new() -> Self {
        Self {
            card: None,
            resolution: Size::new(0, 0),
            queue: None,
        }
    }
}

fn umbreon() -> Structure {
    Structure {
        base: load_image(include_bytes!("../assets/sv8-5_en_161_std.png")),
        foil: Some(load_mask(include_bytes!(
            "../assets/sv8-5_en_161_std.foil.png"
        ))),
        etching: Some(load_mask(include_bytes!(
            "../assets/sv8-5_en_161_std.etch.png"
        ))),
        width: 733,
    }
}

fn load_image(bytes: &[u8]) -> Layer {
    use std::io;

    let mut decoder = png::Decoder::new(io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::ALPHA);

    let mut reader = decoder.read_info().unwrap();
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];

    let metadata = reader.next_frame(&mut rgba).unwrap();
    let bytes = &rgba[..metadata.buffer_size()];

    Layer {
        rgba: Bytes::copy_from_slice(bytes),
        size: metadata.width,
    }
}

fn load_mask(bytes: &[u8]) -> Mask {
    use std::io;

    let decoder = png::Decoder::new(io::Cursor::new(bytes));
    let mut reader = decoder.read_info().unwrap();
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];

    let metadata = reader.next_frame(&mut rgba).unwrap();
    let bytes = &rgba[..metadata.buffer_size()];

    Mask {
        pixels: Bytes::copy_from_slice(bytes),
        size: metadata.width,
    }
}
