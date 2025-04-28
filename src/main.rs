use std::mem;
use std::sync::Arc;

use iced_tiny_skia::Settings;
use iced_widget::Theme;
use iced_widget::core::mouse::{self, Cursor};
use iced_widget::core::renderer::Style;
use iced_widget::core::{Color, Event, Font, Pixels, Point, Size, clipboard, keyboard};
use iced_widget::graphics::{Compositor, Viewport};
use iced_widget::runtime::Debug;
use iced_widget::runtime::program::State;

mod tour;

#[cfg(not(target_os = "helenos"))]
mod platform {
    mod winit;
    pub use winit::*;
}

#[cfg(target_os = "helenos")]
mod platform {
    mod helenos;
    pub use helenos::*;
}

pub type Element<'a, M> =
    iced_widget::core::Element<'a, M, iced_widget::core::Theme, iced_widget::renderer::Renderer>;

struct AppInner {
    w: Arc<platform::Window>,
    surface: iced_tiny_skia::window::Surface,
    compositor: iced_tiny_skia::window::Compositor,
    renderer: iced_tiny_skia::Renderer,

    program: State<tour::Tour>,
    debug: Debug,
}

impl AppInner {
    fn update(&mut self, cursor: Cursor, events: Vec<Event>) {
        let s = self.w.inner_size();

        for e in events {
            self.program.queue_event(e);
        }

        self.program.update(
            Size::new(s.width as f32, s.height as f32),
            cursor,
            &mut self.renderer,
            &Theme::Light,
            &Style::default(),
            &mut clipboard::Null,
            &mut self.debug,
        );

        platform::set_cursor(&self.w, self.program.mouse_interaction());

        self.compositor
            .configure_surface(&mut self.surface, s.width as u32, s.height as u32);
        self.compositor
            .present::<String>(
                &mut self.renderer,
                &mut self.surface,
                &Viewport::with_physical_size(Size::new(s.width, s.height), self.w.scale_factor()),
                Color::WHITE,
                &[],
            )
            .unwrap();
    }

    fn new(w: platform::Window) -> Self {
        let w = Arc::new(w);
        let mut compositor = iced_tiny_skia::window::compositor::new(
            Settings {
                default_font: Font::DEFAULT,
                default_text_size: Pixels(16.0),
            },
            w.clone(),
        );
        let mut renderer = iced_tiny_skia::Renderer::new(Font::DEFAULT, Pixels(10.0));
        let mut debug = Debug::new();
        Self {
            surface: compositor.create_surface(w.clone(), 300, 200),
            compositor,
            w,
            program: State::new(
                crate::tour::Tour::default(),
                Size::new(300.0, 200.0),
                &mut renderer,
                &mut debug,
            ),
            debug,
            renderer,
        }
    }
}

fn main() {
    platform::main();
}
