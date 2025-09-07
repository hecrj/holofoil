use holofoil::{Bytes, Card, Layer, Mask, Pipeline, Structure};

use iced::mouse;
use iced::theme;
use iced::time::{Duration, Instant};
use iced::wgpu;
use iced::widget::{container, row, shader};
use iced::window;
use iced::{Color, Element, Fill, Font, Rectangle, Size, Subscription, Task, Theme};

use std::sync::{Arc, Mutex};

fn main() -> iced::Result {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    #[cfg(not(target_arch = "wasm32"))]
    tracing_subscriber::fmt::init();

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
    .default_font(Font::MONOSPACE)
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

    fn subscription(&self) -> Subscription<Message> {
        window::frames().map(|_| Message::FrameRequested)
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

    fn view(&self) -> Element<'_, Message> {
        row![
            shader(&self.viewer).width(Fill).height(Fill),
            container("Controls").padding(10).width(200).height(Fill)
        ]
        .spacing(10)
        .into()
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

struct Renderer {
    pipeline: Pipeline,
    #[cfg(not(target_arch = "wasm32"))]
    watcher: Watcher,
}

impl shader::Primitive for Holofoil {
    type Renderer = Renderer;

    fn initialize(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self::Renderer {
        #[cfg(not(target_arch = "wasm32"))]
        let watcher = Watcher::new(device, queue, format);

        Renderer {
            pipeline: pipeline(device, queue, format),
            #[cfg(not(target_arch = "wasm32"))]
            watcher,
        }
    }

    fn prepare(
        &self,
        renderer: &mut Renderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &shader::Viewport,
    ) {
        let mut cache = self.cache.lock().unwrap();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(pipeline) = renderer.watcher.latest() {
            renderer.pipeline = pipeline;
            cache.card = None;
        }

        let mut card = cache
            .card
            .take()
            .unwrap_or_else(|| renderer.pipeline.upload(device, queue, &self.card));

        const ROTATION_SPEED: f32 = std::f32::consts::PI / 4.0;

        card.prepare(queue, 0.0, 0.0, ROTATION_SPEED * self.delta.as_secs_f32());
        cache.card = Some(card);

        if let Some(bounds) = (*bounds * viewport.scale_factor()).snap() {
            cache.resolution = Size::new(bounds.width, bounds.height);
            cache.queue = Some(queue.clone());
        }
    }

    fn draw(
        &self,
        renderer: &Renderer,
        render_pass: &mut wgpu::RenderPass<'_>,
        clip_bounds: &Rectangle<u32>,
    ) -> bool {
        let cache = self.cache.lock().unwrap();

        let Some(card) = &cache.card else {
            return true;
        };

        let Some(queue) = &cache.queue else {
            return true;
        };

        render_pass.set_scissor_rect(
            clip_bounds.x,
            clip_bounds.y,
            clip_bounds.width,
            clip_bounds.height,
        );

        renderer.pipeline.render(
            queue,
            render_pass,
            (cache.resolution.width, cache.resolution.height),
            card,
        );

        true
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

fn pipeline(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Pipeline {
    Pipeline::new(
        device,
        queue,
        format,
        load_image(include_bytes!("../assets/pokemon_tcg_back.png")),
    )
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

#[cfg(not(target_arch = "wasm32"))]
struct Watcher {
    _raw: notify_debouncer_full::Debouncer<
        notify_debouncer_full::notify::RecommendedWatcher,
        notify_debouncer_full::RecommendedCache,
    >,
    pipelines: Mutex<std::sync::mpsc::Receiver<Pipeline>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Watcher {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        use notify_debouncer_full::{DebounceEventResult, new_debouncer, notify};
        use std::path::PathBuf;
        use std::sync::mpsc;
        use std::thread;

        let (sender, receiver) = mpsc::channel();
        let device = device.clone();
        let queue = queue.clone();

        let mut debouncer = new_debouncer(
            Duration::from_millis(10),
            None,
            move |events: DebounceEventResult| {
                let Ok(events) = events else {
                    return;
                };

                let modified = events.iter().any(|event| {
                    event.paths.iter().any(|path| path.ends_with("shader.wgsl"))
                        && (event.kind.is_modify()
                            || event.kind.is_remove()
                            || event.kind.is_create())
                });

                if modified {
                    let device = device.clone();
                    let queue = queue.clone();

                    log::info!("Recompiling shader...");

                    if let Ok(pipeline) =
                        thread::spawn(move || pipeline(&device, &queue, format)).join()
                    {
                        let _ = sender.send(pipeline);
                    }
                }
            },
        )
        .unwrap();

        debouncer
            .watch(
                &PathBuf::from(format!("{}/../../src", env!("CARGO_MANIFEST_DIR"))),
                notify::RecursiveMode::NonRecursive,
            )
            .unwrap();

        Self {
            _raw: debouncer,
            pipelines: Mutex::new(receiver),
        }
    }

    fn latest(&mut self) -> Option<Pipeline> {
        self.pipelines.lock().unwrap().try_iter().last()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Watcher {
    fn drop(&mut self) {
        if let Ok(watcher) =
            notify_debouncer_full::new_debouncer(Duration::from_millis(10), None, |_| {})
        {
            self._raw = watcher;
        }

        // This avoids a SIGSEGV because the notifier captures `Device`
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
