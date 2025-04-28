use std::num::NonZeroU32;
use std::rc::Rc;

use iced_widget::core::{Color, Font, Pixels, Size};
use iced_widget::graphics::Viewport;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

mod tour;

pub type Element<'a, M> =
    iced_widget::core::Element<'a, M, iced_widget::core::Theme, iced_widget::renderer::Renderer>;

struct State {
    w: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    app: tour::Tour,
    renderer: iced_tiny_skia::Renderer,
}

struct App {
    state: Option<State>,
}

impl State {
    fn redraw(&mut self, el: &ActiveEventLoop, view: &Element<tour::Message>) {
        let s = self.w.inner_size();
        self.surface
            .resize(
                NonZeroU32::new(s.width).unwrap(),
                NonZeroU32::new(s.height).unwrap(),
            )
            .unwrap();
        let mut buffer = self.surface.buffer_mut().unwrap();
        let sk_surface = iced_tiny_skia::window::Surface
        iced_tiny_skia::window::compositor::present(
            &mut self.renderer,
            &mut self.surface,
            &Viewport::with_physical_size(Size::new(s.width, s.height), self.w.scale_factor()),
            Color::BLACK,
            &[],
        );
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
            app: tour::Tour::default(),
            renderer: iced_tiny_skia::Renderer::new(Font::DEFAULT, Pixels(10.0)),
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
