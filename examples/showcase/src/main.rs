use holofoil::card;
use holofoil::{Bytes, Card, Configuration, Pipeline, Quaternion, Vector};

use iced::mouse;
use iced::theme;
use iced::time::{Duration, Instant};
use iced::wgpu;
use iced::widget::{
    bottom_right, column, container, horizontal_space, opaque, row, shader, stack, text, toggler,
};
use iced::window;
use iced::{
    Center, Color, Element, Event, Fill, Font, Point, Rectangle, Subscription, Task, Theme,
    Vector as Vector2,
};
use iced_palace::widget::labeled_slider;

use std::f32::consts::{FRAC_PI_4, PI};
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
    .theme(Theme::CatppuccinMocha)
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
    Spinning { spin: Vector2, last_tick: Instant },
}

impl Mode {
    fn spin(now: Instant) -> Self {
        Self::Spinning {
            spin: Vector2::new(FRAC_PI_4, 0.0),
            last_tick: now,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Booted,
    FrameRequested,
    Grabbed,
    RotationChanged(Quaternion),
    RotationEulerChanged(Vector),
    ToggleAutoRotate(bool),
    Spin(Vector2),
    SamplesChanged(u32),
    MaxIterationsChanged(u32),
}

impl Showcase {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                viewer: Viewer {
                    card: Arc::new(umbreon()),
                    cache: Arc::new(Mutex::new(Cache::new())),
                    configuration: Configuration::default(),
                    rotation: Quaternion::default(),
                    euler: Vector::default(),
                },
                mode: Mode::Idle,
            },
            Task::done(Message::Booted),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        match self.mode {
            Mode::Idle => Subscription::none(),
            Mode::Spinning { .. } => window::frames().map(|_| Message::FrameRequested),
        }
    }

    fn update(&mut self, message: Message, now: Instant) {
        match message {
            Message::Booted => self.mode = Mode::spin(now),
            Message::FrameRequested => match &mut self.mode {
                Mode::Spinning { spin, last_tick } => {
                    let delta = *spin * now.duration_since(*last_tick).as_secs_f32();

                    let spin = Quaternion::from_radians(Vector::X, delta.y)
                        * Quaternion::from_radians(Vector::Y, delta.x);

                    self.viewer.rotation = (spin * self.viewer.rotation).normalize();
                    self.viewer.euler = self.viewer.rotation.to_euler();

                    *last_tick = now;
                }
                Mode::Idle => {}
            },
            Message::Grabbed => {
                self.mode = Mode::Idle;
            }
            Message::RotationChanged(rotation) => {
                self.viewer.rotation = rotation;
                self.viewer.euler = rotation.to_euler();
            }
            Message::RotationEulerChanged(rotation) => {
                self.viewer.rotation = Quaternion::from_radians(Vector::Y, rotation.y)
                    * Quaternion::from_radians(Vector::X, rotation.x)
                    * Quaternion::from_radians(Vector::Z, rotation.z);

                self.viewer.euler = rotation;
                self.mode = Mode::Idle;
            }
            Message::ToggleAutoRotate(auto_rotate) => {
                self.mode = if auto_rotate {
                    Mode::spin(now)
                } else {
                    Mode::Idle
                };
            }
            Message::Spin(spin) => {
                self.mode = Mode::Spinning {
                    spin,
                    last_tick: now,
                };
            }
            Message::SamplesChanged(n_samples) => {
                self.viewer.configuration.n_samples = n_samples;
            }
            Message::MaxIterationsChanged(max_iterations) => {
                self.viewer.configuration.max_iterations = max_iterations;
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let controls = [self.quality(), self.rotation()];

        stack![
            shader(&self.viewer).width(Fill).height(Fill),
            bottom_right(opaque(column(controls).width(230).spacing(10).padding(10)))
        ]
        .into()
    }

    fn quality(&self) -> Element<'_, Message> {
        let Configuration {
            n_samples,
            max_iterations,
        } = self.viewer.configuration;

        control(
            "Quality",
            column![
                labeled_slider(
                    "Samples",
                    (1..=8, 1),
                    n_samples,
                    Message::SamplesChanged,
                    u32::to_string,
                ),
                labeled_slider(
                    "Segments",
                    (32..=256, 1),
                    max_iterations,
                    Message::MaxIterationsChanged,
                    u32::to_string,
                ),
            ]
            .spacing(5),
        )
    }

    fn rotation(&self) -> Element<'_, Message> {
        let rotation_slider = |label, get: fn(Vector) -> f32, set: fn(Vector, f32) -> Vector| {
            labeled_slider(
                label,
                (0.0..=359.99, 0.1),
                get(self.viewer.euler).to_degrees().rem_euclid(360.0),
                move |angle| {
                    Message::RotationEulerChanged(set(self.viewer.euler, angle.to_radians()))
                },
                |angle| format!("{angle:.2}°"),
            )
        };

        control_with_toggle(
            "Rotation",
            matches!(self.mode, Mode::Spinning { .. }),
            Message::ToggleAutoRotate,
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
        )
    }
}

fn control<'a>(
    title: impl text::IntoFragment<'a>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(column![text(title).size(14), content.into()].spacing(10))
        .width(Fill)
        .padding(10)
        .style(container::bordered_box)
        .into()
}

fn control_with_toggle<'a>(
    title: impl text::IntoFragment<'a>,
    is_toggled: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(
        column![
            row![
                text(title).size(14),
                horizontal_space(),
                toggler(is_toggled).spacing(5).on_toggle(on_toggle)
            ]
            .spacing(10)
            .align_y(Center),
            content.into()
        ]
        .spacing(10),
    )
    .width(Fill)
    .padding(10)
    .style(container::bordered_box)
    .into()
}

#[derive(Debug)]
struct Viewer {
    card: Arc<card::Structure>,
    cache: Arc<Mutex<Cache>>,
    configuration: Configuration,
    rotation: Quaternion,
    euler: Vector,
}

#[derive(Debug, Default)]
enum Interaction {
    #[default]
    Idle,
    Dragging {
        origin: Quaternion,
        start: Point,
        last: Point,
        now: Instant,
        speed: Vector2,
    },
}

impl shader::Program<Message> for Viewer {
    type State = Interaction;
    type Primitive = Holofoil;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<shader::Action<Message>> {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let start = cursor.position_over(bounds)?;

                *state = Interaction::Dragging {
                    origin: self.rotation,
                    start,
                    last: start,
                    now: Instant::now(),
                    speed: Vector2::ZERO,
                };

                Some(shader::Action::publish(Message::Grabbed))
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let interaction = std::mem::take(state);

                let Interaction::Dragging { speed, now, .. } = interaction else {
                    return None;
                };

                let factor = (Point::ORIGIN + speed).distance(Point::ORIGIN);

                if factor < 100.0 || now.elapsed().as_millis() > 200 {
                    return None;
                }

                let scale = bounds.width.min(bounds.height);

                Some(shader::Action::publish(Message::Spin(speed * (PI / scale))))
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let Interaction::Dragging {
                    origin,
                    start,
                    last,
                    now,
                    speed,
                } = state
                else {
                    return None;
                };

                let current = cursor.land().position()?;
                let delta = current - *start;
                let scale = bounds.width.min(bounds.height);

                let rotation = Quaternion::from_radians(Vector::X, PI * delta.y / scale)
                    * Quaternion::from_radians(Vector::Y, PI * delta.x / scale);

                let duration = now.elapsed();

                if duration.as_millis() > 10 {
                    *now = Instant::now();
                    *speed = (*speed + (current - *last) * (1.0 / duration.as_secs_f32())) * 0.5;
                    *last = current;
                }

                Some(shader::Action::publish(Message::RotationChanged(
                    rotation * *origin,
                )))
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        Holofoil {
            card: self.card.clone(),
            cache: self.cache.clone(),
            configuration: self.configuration,
            rotation: self.rotation,
        }
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match state {
            Interaction::Idle => mouse::Interaction::None,
            Interaction::Dragging { .. } => mouse::Interaction::Grabbing,
        }
    }
}

#[derive(Debug)]
struct Holofoil {
    card: Arc<card::Structure>,
    cache: Arc<Mutex<Cache>>,
    configuration: Configuration,
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

        renderer.pipeline.configure(queue, self.configuration);

        card.prepare(
            queue,
            card::Parameters {
                viewport: card::Viewport {
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

fn umbreon() -> card::Structure {
    card::Structure {
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

fn load_image(bytes: &[u8]) -> card::Image {
    use std::io;

    let mut decoder = png::Decoder::new(io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::ALPHA);

    let mut reader = decoder.read_info().unwrap();
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];

    let metadata = reader.next_frame(&mut rgba).unwrap();
    let bytes = &rgba[..metadata.buffer_size()];

    card::Image {
        rgba: Bytes::copy_from_slice(bytes),
        size: metadata.width,
    }
}

fn load_mask(bytes: &[u8]) -> card::Mask {
    use std::io;

    let decoder = png::Decoder::new(io::Cursor::new(bytes));
    let mut reader = decoder.read_info().unwrap();
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];

    let metadata = reader.next_frame(&mut rgba).unwrap();
    let bytes = &rgba[..metadata.buffer_size()];

    card::Mask {
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
