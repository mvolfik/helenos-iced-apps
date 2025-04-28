use std::sync::Arc;

use iced_tiny_skia::Settings;
use iced_widget::Theme;
use iced_widget::core::mouse::Cursor;
use iced_widget::core::renderer::Style;
use iced_widget::core::{Color, Font, Pixels, Size, clipboard};
use iced_widget::graphics::{Compositor, Viewport};
use iced_widget::runtime::Debug;
use iced_widget::runtime::program::State;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

mod tour;

pub type Element<'a, M> =
    iced_widget::core::Element<'a, M, iced_widget::core::Theme, iced_widget::renderer::Renderer>;

struct AppInner {
    w: Arc<Window>,
    surface: iced_tiny_skia::window::Surface,
    compositor: iced_tiny_skia::window::Compositor,
    renderer: iced_tiny_skia::Renderer,

    program: State<tour::Tour>,
    debug: Debug,
}

struct App {
    inner: Option<AppInner>,
    cursor: Cursor,
}

impl AppInner {
    fn update(&mut self, cursor: Cursor) {
        let s = self.w.inner_size();

        self.program.update(
            Size::new(s.width as f32, s.height as f32),
            cursor,
            &mut self.renderer,
            &Theme::Light,
            &Style::default(),
            &mut clipboard::Null,
            &mut self.debug,
        );

        self.compositor
            .present::<String>(
                &mut self.renderer,
                &mut self.surface,
                &Viewport::with_physical_size(Size::new(s.width, s.height), self.w.scale_factor()),
                Color::BLACK,
                &[],
            )
            .unwrap();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        println!("Resumed");
        let w = Arc::new(el.create_window(Window::default_attributes()).unwrap());
        let mut compositor = iced_tiny_skia::window::compositor::new(
            Settings {
                default_font: Font::DEFAULT,
                default_text_size: Pixels(16.0),
            },
            w.clone(),
        );
        let mut renderer = iced_tiny_skia::Renderer::new(Font::DEFAULT, Pixels(10.0));
        let mut debug = Debug::new();
        self.inner = Some(AppInner {
            surface: compositor.create_surface(w.clone(), 300, 200),
            compositor,
            w,
            program: State::new(
                tour::Tour::default(),
                Size::new(300.0, 200.0),
                &mut renderer,
                &mut debug,
            ),
            debug,
            renderer,
        });
        self.inner.as_mut().unwrap().update(Cursor::Unavailable);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _wid: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                el.exit();
            }
            WindowEvent::RedrawRequested => {
                println!("Redraw requested");
                self.inner.as_mut().unwrap().update(w);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                println!("Keyboard input: {:?}", event);
                self.inner.as_ref().unwrap().w.request_redraw();
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

    el.run_app(&mut App { inner: None }).unwrap();
}
