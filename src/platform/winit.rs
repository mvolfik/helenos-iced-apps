use std::mem;
use std::sync::Arc;

use iced_widget::core::mouse::{self, Cursor, Interaction};
use iced_widget::core::{Event, Point, keyboard};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::WindowId;

pub use winit::window::Window;

use crate::AppInner;

pub struct App {
    inner: Option<AppInner>,
    cursor: Cursor,
    events_cache: Vec<Event>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        self.inner = Some(AppInner::new(Arc::new(
            el.create_window(Window::default_attributes()).unwrap(),
        )));
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
            _ => {}
        }
        if !is_redraw {
            self.inner.as_ref().unwrap().w.request_redraw();
        }
    }
}

pub fn set_cursor(w: &Window, interaction: Interaction) {
    w.set_cursor(winit::window::Cursor::Icon(match interaction {
        iced_widget::core::mouse::Interaction::Pointer => winit::window::CursorIcon::Pointer,
        iced_widget::core::mouse::Interaction::Grab => winit::window::CursorIcon::Grabbing,
        _ => winit::window::CursorIcon::Default,
    }));
}

pub fn main() {
    let el = EventLoop::new().unwrap();

    el.run_app(&mut App {
        inner: None,
        cursor: Cursor::Unavailable,
        events_cache: Vec::new(),
    })
    .unwrap();
}
