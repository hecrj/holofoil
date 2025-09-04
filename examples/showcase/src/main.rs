use holofoil::wgpu::{self, Adapter, Device, Queue, Surface, TextureFormat};
use holofoil::{Bytes, Card, Layer, Pipeline, Structure};

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

use std::sync::Arc;
use web_time::Instant;

pub fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    let event_loop = EventLoop::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();

    #[cfg(not(target_arch = "wasm32"))]
    {
        use notify::Watcher as _;
        use std::path::PathBuf;

        let proxy = proxy.clone();

        let mut watcher =
            notify::recommended_watcher(move |event: Result<notify::Event, notify::Error>| {
                let Ok(event) = event else {
                    return;
                };

                if !event.paths.iter().any(|path| path.ends_with("shader.wgsl")) {
                    return;
                }

                let (notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
                | notify::EventKind::Remove(notify::event::RemoveKind::File)) = event.kind
                else {
                    return;
                };

                proxy.send_event(Event::Reload).unwrap();
            })
            .unwrap();
        watcher
            .watch(
                &PathBuf::from(format!("{}/src", env!("CARGO_MANIFEST_DIR"))),
                notify::RecursiveMode::NonRecursive,
            )
            .unwrap();
    }

    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop
        .run_app(&mut Showcase::Loading { proxy })
        .unwrap();
}

#[derive(Debug)]
enum Showcase {
    Loading {
        proxy: EventLoopProxy<Event>,
    },
    Ready {
        device: Device,
        queue: Queue,
        surface: Surface<'static>,
        format: TextureFormat,
        pipeline: Pipeline,
        card: Card,
        start: Instant,
        window: Arc<Window>,
        size: PhysicalSize<u32>,
    },
}

#[derive(Debug)]
enum Event {
    Boot(Boot),
    #[cfg(not(target_arch = "wasm32"))]
    Reload,
}

#[derive(Debug)]
struct Boot {
    adapter: Adapter,
    device: Device,
    queue: Queue,
    surface: Surface<'static>,
    window: Arc<Window>,
}

impl ApplicationHandler<Event> for Showcase {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Self::Loading { proxy } = self else {
            return;
        };

        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("Holofoil - Iced"))
                .unwrap(),
        );

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowExtWebSys;

            let canvas = window.canvas().expect("Get window canvas");
            let _ = canvas.set_attribute("style", "display: block; width: 100%; height: 100%");

            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();
            let body = document.body().unwrap();

            let _ = body
                .append_child(&canvas)
                .expect("Append canvas to HTML body");
        }

        let proxy = proxy.clone();

        let boot = async move {
            let instance = wgpu::util::new_instance_with_webgpu_detection(
                &wgpu::InstanceDescriptor::default(),
            )
            .await;

            let surface = instance.create_surface(window.clone()).unwrap();

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    ..wgpu::RequestAdapterOptions::default()
                })
                .await
                .unwrap();

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    ..wgpu::DeviceDescriptor::default()
                })
                .await
                .unwrap();

            proxy
                .send_event(Event::Boot(Boot {
                    adapter,
                    device,
                    queue,
                    surface,
                    window,
                }))
                .unwrap();
        };

        #[cfg(not(target_arch = "wasm32"))]
        {
            use pollster::FutureExt;
            boot.block_on();
        }

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(boot);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if event_loop.exiting() {
            return;
        }

        let Self::Ready {
            window,
            device,
            queue,
            size,
            surface,
            pipeline,
            card,
            start,
            ..
        } = self
        else {
            return;
        };

        match event {
            WindowEvent::Resized(new_size) => {
                *size = new_size;

                self.configure_surface();
            }
            WindowEvent::RedrawRequested => {
                if size.width == 0 || size.height == 0 {
                    return;
                }

                let frame = surface.get_current_texture().unwrap();

                const ROTATION_SPEED: f32 = std::f32::consts::PI / 4.0;

                card.prepare(
                    queue,
                    0.0,
                    0.0,
                    ROTATION_SPEED * start.elapsed().as_secs_f32(),
                );

                let mut encoder =
                    device.create_command_encoder(&wgpu::wgt::CommandEncoderDescriptor::default());

                {
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &frame
                                .texture
                                .create_view(&wgpu::TextureViewDescriptor::default()),
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        ..wgpu::RenderPassDescriptor::default()
                    });

                    pipeline.render(queue, &mut render_pass, (size.width, size.height), card);
                }

                queue.submit([encoder.finish()]);
                window.pre_present_notify();
                frame.present();

                window.request_redraw();
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        if event_loop.exiting() {
            return;
        }

        match event {
            Event::Boot(Boot {
                adapter,
                device,
                queue,
                surface,
                window,
            }) => {
                let size = window.inner_size();
                let format = surface.get_capabilities(&adapter).formats[0];

                let pipeline = Pipeline::new(
                    &device,
                    &queue,
                    format,
                    load_image(include_bytes!("../assets/pokemon_tcg_back.png")),
                );

                let card = pipeline.upload(&device, &queue, umbreon()).unwrap();

                window.request_redraw();

                *self = Self::Ready {
                    device,
                    queue,
                    window,
                    size,
                    surface,
                    format,
                    pipeline,
                    card,
                    start: Instant::now(),
                };

                self.configure_surface();
            }
            #[cfg(not(target_arch = "wasm32"))]
            Event::Reload => {
                let Self::Ready {
                    device,
                    queue,
                    format,
                    pipeline,
                    card,
                    ..
                } = self
                else {
                    return;
                };

                *pipeline = Pipeline::new(
                    device,
                    queue,
                    *format,
                    load_image(include_bytes!("../assets/pokemon_tcg_back.png")),
                );

                *card = pipeline.upload(device, queue, umbreon()).unwrap();
            }
        }
    }
}

impl Showcase {
    fn configure_surface(&mut self) {
        let Self::Ready {
            device,
            surface,
            format,
            size,
            ..
        } = self
        else {
            return;
        };

        if size.width == 0 || size.height == 0 {
            return;
        }

        surface.configure(
            &device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: *format,
                width: size.width,
                height: size.height,
                present_mode: wgpu::PresentMode::default(),
                desired_maximum_frame_latency: 1,
                alpha_mode: wgpu::CompositeAlphaMode::default(),
                view_formats: vec![],
            },
        );
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

fn umbreon() -> Structure {
    Structure {
        base: load_image(include_bytes!("../assets/sv8-5_en_161_std.png")),
        foil: Some(load_image(include_bytes!(
            "../assets/sv8-5_en_161_std.foil.png"
        ))),
        etching: Some(load_image(include_bytes!(
            "../assets/sv8-5_en_161_std.etch.png"
        ))),
        width: 733,
    }
}
