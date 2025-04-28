use std::num::NonZeroU32;
use std::rc::Rc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

struct State {
    w: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
}

struct App {
    state: Option<State>,
}

impl State {
    fn redraw(&mut self, el: &ActiveEventLoop) {
        let s = self.w.inner_size();
        self.surface
            .resize(
                NonZeroU32::new(s.width).unwrap(),
                NonZeroU32::new(s.height).unwrap(),
            )
            .unwrap();
        let mut buffer = self.surface.buffer_mut().unwrap();
        for y in 0..s.height {
            for x in 0..s.width {
                let red = x % 255;
                let green = y % 255;
                let blue = (y * 100 / 255 + x * 200 / 255) % 255;

                buffer[(y * s.width + x) as usize] = blue | (green << 8) | (red << 16);
            }
        }
        buffer.present().unwrap();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        println!("Resumed");
        let w = Rc::new(el.create_window(Window::default_attributes()).unwrap());
        self.state = Some(State {
            surface: softbuffer::Surface::new(
                &softbuffer::Context::new(w.clone()).unwrap(),
                w.clone(),
            )
            .unwrap(),
            w,
        });
        self.state.as_mut().unwrap().redraw(el);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _wid: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                el.exit();
            }
            WindowEvent::RedrawRequested => {
                println!("Redraw requested");
                self.state.as_mut().unwrap().redraw(el);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                println!("Keyboard input: {:?}", event);
                self.state.as_ref().unwrap().w.request_redraw();
            }
            WindowEvent::MouseInput { .. }
            | WindowEvent::AxisMotion { .. }
            | WindowEvent::CursorMoved { .. } => {}
            e => {
                println!("Window event: {:?}", e);
            }
        }
    }
}
fn main() {
    let el = EventLoop::new().unwrap();

    el.run_app(&mut App { state: None }).unwrap();
}
