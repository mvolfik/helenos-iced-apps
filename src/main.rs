#![feature(never_type)]
#![feature(unwrap_infallible)]
#![feature(float_minimum_maximum)]

use std::borrow::Cow;
use std::fmt::Debug;
use std::sync::Arc;

use iced_runtime::Program;
use iced_tiny_skia::Settings;
use iced_widget::core::mouse::Cursor;
use iced_widget::core::renderer::Style;
use iced_widget::core::{Color, Event, Pixels, Size, clipboard, font};
use iced_widget::graphics::{Compositor, Viewport};
use iced_widget::runtime::program::State;
use iced_widget::{Renderer, Theme};

mod viewer;

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

pub type Element<'a, M> = iced_widget::core::Element<'a, M, Theme, Renderer>;

struct AppInner<T: Program + 'static> {
    w: Arc<platform::Window>,
    surface: iced_tiny_skia::window::Surface,
    compositor: iced_tiny_skia::window::Compositor,
    renderer: Renderer,

    program: State<T>,
    debug: iced_widget::runtime::Debug,
}

impl<T> Debug for AppInner<T>
where
    T: Debug + Program<Theme = Theme, Renderer = Renderer> + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppInner")
            .field("w", &self.w)
            .field("program", &self.program.program())
            .finish_non_exhaustive()
    }
}

impl<T> AppInner<T>
where
    T: Debug + Program<Renderer = Renderer, Theme = Theme> + 'static,
{
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

    fn new(w: Arc<platform::Window>, create_app: impl FnOnce() -> T) -> Self {
        let mut compositor = iced_tiny_skia::window::compositor::new(
            Settings {
                default_font: font::Font {
                    family: font::Family::Name("Noto Sans"),
                    ..Default::default()
                },
                default_text_size: Pixels(12.0),
            },
            w.clone(),
        );
        compositor.load_font(Cow::Borrowed(include_bytes!("../NotoSans-Regular.ttf")));
        compositor.load_font(Cow::Borrowed(include_bytes!("../NotoSansMono-Regular.ttf")));

        let mut renderer = compositor.create_renderer();
        let mut debug = iced_widget::runtime::Debug::new();
        Self {
            surface: compositor.create_surface(w.clone(), 300, 200),
            compositor,
            w,
            program: State::new(
                create_app(),
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
    platform::main(
        || viewer::Viewer::new(std::env::args().nth(1)),
        "Image viewer.rs",
    );
}
