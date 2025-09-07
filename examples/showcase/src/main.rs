use holofoil::{
    Bytes, Card, Layer, Mask, Parameters, Pipeline, Quaternion, Structure, Vector, Viewport,
};

use iced::mouse;
use iced::theme;
use iced::time::{Duration, Instant};
use iced::wgpu;
use iced::widget::{
    bottom_right, column, container, horizontal_space, row, shader, stack, text, toggler,
};
use iced::window;
use iced::{Center, Color, Element, Fill, Font, Rectangle, Subscription, Task, Theme};
use iced_palace::widget::labeled_slider;

use std::f32::consts::FRAC_PI_4;
use std::sync::{Arc, Mutex};

fn main() -> iced::Result {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    #[cfg(not(target_arch = "wasm32"))]
    tracing_subscriber::fmt::init();

    let application = iced::application::timed(
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
    .default_font(Font::MONOSPACE);

    #[cfg(target_arch = "wasm32")]
    {
        application
            .font(include_bytes!("../fonts/RobotoMono-Regular.ttf"))
            .default_font(Font {
                family: iced::font::Family::Name("Roboto Mono"),
                ..Default::default()
            })
            .run()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        application.run()
    }
}

#[derive(Debug)]
struct Showcase {
    viewer: Viewer,
    mode: Mode,
}

#[derive(Debug)]
enum Mode {
    Idle,
    AutoRotate { last_tick: Instant },
}

#[derive(Debug, Clone)]
enum Message {
    Booted,
    FrameRequested,
    RotationChanged(Vector),
    ToggleAutoRotate(bool),
}

impl Showcase {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                viewer: Viewer {
                    card: Arc::new(umbreon()),
                    cache: Arc::new(Mutex::new(Cache::new())),
                    rotation: Quaternion::default(),
                    euler: Vector::default(),
                },
                mode: Mode::Idle,
            },
            Task::done(Message::Booted),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        window::frames().map(|_| Message::FrameRequested)
    }

    fn update(&mut self, message: Message, now: Instant) {
        match message {
            Message::Booted => {
                self.mode = Mode::AutoRotate { last_tick: now };
            }
            Message::FrameRequested => {
                if let Mode::AutoRotate { last_tick } = &mut self.mode {
                    const ROTATION_SPEED: f32 = FRAC_PI_4;

                    let delta = ROTATION_SPEED * now.duration_since(*last_tick).as_secs_f32();

                    self.viewer.rotation = (Quaternion::from_radians(Vector::Y, delta)
                        * self.viewer.rotation)
                        .normalize();

                    self.viewer.euler.y += delta;

                    *last_tick = now;
                }
            }
            Message::RotationChanged(rotation) => {
                self.viewer.rotation = Quaternion::from_radians(Vector::Y, rotation.y)
                    * Quaternion::from_radians(Vector::X, rotation.x)
                    * Quaternion::from_radians(Vector::Z, rotation.z);

                self.viewer.euler = rotation;
                self.mode = Mode::Idle;
            }
            Message::ToggleAutoRotate(auto_rotate) => {
                self.mode = if auto_rotate {
                    Mode::AutoRotate { last_tick: now }
                } else {
                    Mode::Idle
                };
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        stack![
            shader(&self.viewer).width(Fill).height(Fill),
            bottom_right(
                container(self.controls())
                    .width(180)
                    .padding(10)
                    .style(container::bordered_box)
            )
            .padding(10)
        ]
        .into()
    }

    fn controls(&self) -> Element<'_, Message> {
        let rotation_slider = |label, get: fn(Vector) -> f32, set: fn(Vector, f32) -> Vector| {
            labeled_slider(
                label,
                (0.0..=359.99, 0.1),
                get(self.viewer.euler).to_degrees().rem_euclid(360.0),
                move |angle| Message::RotationChanged(set(self.viewer.euler, angle.to_radians())),
                |angle| format!("{angle:.2}°"),
            )
        };

        column![
            row![
                text("Rotation").size(14),
                horizontal_space(),
                toggler(matches!(self.mode, Mode::AutoRotate { .. }))
                    .text_size(14)
                    .spacing(5)
                    .on_toggle(Message::ToggleAutoRotate)
            ]
            .align_y(Center),
            column![
                rotation_slider(
                    "X",
                    |rotation| rotation.x,
                    |rotation, x| Vector { x, ..rotation }
                ),
                rotation_slider(
                    "Y",
                    |rotation| rotation.y,
                    |rotation, y| Vector { y, ..rotation }
                ),
                rotation_slider(
                    "Z",
                    |rotation| rotation.z,
                    |rotation, z| Vector { z, ..rotation }
                )
            ]
            .spacing(5),
        ]
        .spacing(10)
        .into()
    }
}

#[derive(Debug)]
struct Viewer {
    card: Arc<Structure>,
    cache: Arc<Mutex<Cache>>,
    rotation: Quaternion,
    euler: Vector,
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
            rotation: self.rotation,
        }
    }
}

#[derive(Debug)]
struct Holofoil {
    card: Arc<Structure>,
    cache: Arc<Mutex<Cache>>,
    rotation: Quaternion,
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

        let Some(bounds) = (*bounds * viewport.scale_factor()).snap() else {
            return;
        };

        card.prepare(
            queue,
            Parameters {
                viewport: Viewport {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width,
                    height: bounds.height,
                },
                rotation: self.rotation,
            },
        );
        cache.card = Some(card);
    }

    fn draw(&self, renderer: &Renderer, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        let cache = self.cache.lock().unwrap();

        let Some(card) = &cache.card else {
            return true;
        };

        renderer.pipeline.render(render_pass, card);

        true
    }
}

#[derive(Debug)]
struct Cache {
    card: Option<Card>,
}

impl Cache {
    fn new() -> Self {
        Self { card: None }
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
