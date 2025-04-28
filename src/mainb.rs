use iced_tiny_skia::Settings;
use iced_widget::core::{Font, Pixels, Size, Theme, clipboard, mouse::Cursor, renderer::Style};
use iced_widget::graphics::Compositor;
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle,
};

use tour::Message;
use winit::window::{WindowAttributes, WindowId};

mod tour;

struct Window<'a> {
    w: &'a winit::window::Window,
    dh: DisplayHandle<'a>,
}

unsafe impl Send for Window<'_> {}
unsafe impl Sync for Window<'_> {}

impl<'a> HasWindowHandle for Window<'_> {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        self.dh
    }
}

impl HasDisplayHandle for Window<'_> {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.dh.display_handle()
    }
}

pub fn main() {
    tracing_subscriber::fmt::init();

    let mut app = tour::Tour::default();

    let ev = winit::event_loop::EventLoop::new().unwrap();
    let w = ev.create_window(WindowAttributes::new()).unwrap();

    let comp = iced_tiny_skia::window::compositor::new(
        Settings {
            default_font: Font::DEFAULT,
            default_text_size: Pixels(16.0),
        },
        Window {
            w: &w,
            dh: ev.display_handle().unwrap(),
        },
    );

    let h = ev.owned_display_handle();
    ev.run(move |e, el| {});

    // loop {
    //     let interface = app.view();
    // }

    // let bounds = Size::new(100.0, 100.0);
    // let s = comp.create_surface(
    // let renderer = comp.create_renderer();
}
