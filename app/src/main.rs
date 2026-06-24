#![forbid(unsafe_code)]

use std::sync::Arc;
use std::thread;

use grafik_renderer::{Image, render_loop};
use pixels::{Error, Pixels, SurfaceTexture};
use triple_buffer::triple_buffer;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::KeyCode;
use winit::window::Window;
use winit_input_helper::WinitInputHelper;

const WIDTH: u32 = 600;
const HEIGHT: u32 = 600;

fn main() -> Result<(), Error> {
    let event_loop = EventLoop::new().unwrap();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        #[allow(deprecated)]
        Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Grafik")
                        .with_inner_size(size)
                        .with_min_inner_size(size),
                )
                .unwrap(),
        )
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(WIDTH, HEIGHT, surface_texture)?
    };
    let (img_input, mut img_output) = triple_buffer(&Image::new(WIDTH, HEIGHT));
    thread::spawn(move || {
        render_loop(img_input);
    });

    #[allow(deprecated)]
    let res = event_loop.run(|event, elwt| {
        match event {
            Event::Resumed => {
                window.request_redraw();
            }
            Event::NewEvents(_) => input.step(),
            Event::AboutToWait => {
                input.end_step();
                window.request_redraw();
            }
            Event::DeviceEvent { event, .. } => {
                input.process_device_event(&event);
            }
            Event::WindowEvent { event, .. } => {
                // Handle input events
                if input.process_window_event(&event) {
                    // Close events
                    if input.key_pressed(KeyCode::Escape) || input.close_requested() {
                        elwt.exit();
                        return;
                    }
                }

                // Draw the current frame
                if event == WindowEvent::RedrawRequested {
                    if img_output.update() {
                        let img = img_output.output_buffer();
                        img.write_out(pixels.frame_mut());
                        if let Err(err) = pixels.render() {
                            println!("{:?}", err);
                            elwt.exit();
                            return;
                        }
                    }
                }
            }
            _ => {}
        }
    });
    res.map_err(|e| Error::UserDefined(Box::new(e)))
}
