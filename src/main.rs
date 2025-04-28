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
    events_cache: Vec<Event>,
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

        self.w.set_cursor(winit::window::Cursor::Icon(
            match self.program.mouse_interaction() {
                iced_widget::core::mouse::Interaction::Pointer => {
                    winit::window::CursorIcon::Pointer
                }
                iced_widget::core::mouse::Interaction::Grab => winit::window::CursorIcon::Grabbing,
                _ => winit::window::CursorIcon::Default,
            },
        ));

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
}

impl App {}

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
        self.inner
            .as_mut()
            .unwrap()
            .update(self.cursor, mem::take(&mut self.events_cache));
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _wid: WindowId, event: WindowEvent) {
        let is_redraw = event == WindowEvent::RedrawRequested;
        match event {
            WindowEvent::CursorLeft { .. } => {
                self.cursor = Cursor::Unavailable;
                self.events_cache
                    .push(Event::Mouse(mouse::Event::CursorLeft));
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = Point::new(position.x as f32, position.y as f32);
                self.cursor = Cursor::Available(p);
                self.events_cache
                    .push(Event::Mouse(mouse::Event::CursorMoved { position: p }));
            }
            WindowEvent::CloseRequested => {
                el.exit();
            }
            WindowEvent::RedrawRequested => {
                self.inner
                    .as_mut()
                    .unwrap()
                    .update(self.cursor, mem::take(&mut self.events_cache));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let ch = keyboard::Key::Character(event.text.clone().unwrap_or_default());
                self.events_cache.push(Event::Keyboard(match event.state {
                    winit::event::ElementState::Pressed => keyboard::Event::KeyPressed {
                        key: ch.clone(),
                        modified_key: ch,
                        physical_key: keyboard::key::Physical::Unidentified(
                            keyboard::key::NativeCode::Unidentified,
                        ),
                        location: keyboard::Location::Standard,
                        modifiers: keyboard::Modifiers::default(),
                        text: event.text,
                    },
                    winit::event::ElementState::Released => keyboard::Event::KeyReleased {
                        key: ch,
                        location: keyboard::Location::Standard,
                        modifiers: keyboard::Modifiers::default(),
                    },
                }));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.events_cache
                    .push(Event::Mouse(mouse::Event::WheelScrolled {
                        delta: match delta {
                            winit::event::MouseScrollDelta::LineDelta(x, y) => {
                                mouse::ScrollDelta::Lines {
                                    x: x as f32,
                                    y: y as f32,
                                }
                            }
                            winit::event::MouseScrollDelta::PixelDelta(physical_position) => {
                                mouse::ScrollDelta::Pixels {
                                    x: physical_position.x as f32,
                                    y: physical_position.y as f32,
                                }
                            }
                        },
                    }))
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    winit::event::MouseButton::Left => mouse::Button::Left,
                    winit::event::MouseButton::Right => mouse::Button::Right,
                    winit::event::MouseButton::Middle => mouse::Button::Middle,
                    _ => mouse::Button::Other(99),
                };
                self.events_cache.push(Event::Mouse(match state {
                    winit::event::ElementState::Pressed => mouse::Event::ButtonPressed(button),
                    winit::event::ElementState::Released => mouse::Event::ButtonReleased(button),
                }));
            }

            e => {}
        }
        if !is_redraw {
            self.inner.as_ref().unwrap().w.request_redraw();
        }
    }
}
fn main() {
    let el = EventLoop::new().unwrap();

    el.run_app(&mut App {
        inner: None,
        cursor: Cursor::Unavailable,
        events_cache: Vec::new(),
    })
    .unwrap();
}
