use std::sync::Arc;

use iced_widget::core::mouse::{self, Cursor, Interaction};
use iced_widget::core::{Event, Point, keyboard};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::WindowId;

pub use winit::window::Window;

use crate::{AppInner, ProgramExt, SendMsgFn, WindowOptions};

pub struct App<T: ProgramExt> {
    inner: Option<AppInner<T>>,
    cursor: Cursor,
    // app that we will run - we store it here until the first Resume event
    prepared_app: Option<(T, WindowOptions)>,
}

impl<T: ProgramExt> ApplicationHandler<T::Message> for App<T> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if let Some((app, options)) = self.prepared_app.take() {
            self.inner = Some(AppInner::new(
                Arc::new(
                    el.create_window(
                        Window::default_attributes()
                            .with_title(options.caption)
                            .with_maximized(options.maximized),
                    )
                    .unwrap(),
                ),
                app,
            ));
        }
        self.inner.as_mut().unwrap().update(self.cursor);
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, msg: T::Message) {
        if let Some(inner) = self.inner.as_mut() {
            inner.program.queue_message(msg);
            inner.w.request_redraw();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _wid: WindowId, event: WindowEvent) {
        let new_ev = match event {
            WindowEvent::CursorLeft { .. } => {
                self.cursor = Cursor::Unavailable;
                Event::Mouse(mouse::Event::CursorLeft)
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = Point::new(position.x as f32, position.y as f32);
                self.cursor = Cursor::Available(p);
                Event::Mouse(mouse::Event::CursorMoved { position: p })
            }
            WindowEvent::CloseRequested => {
                el.exit();
                self.inner.as_mut().unwrap().program.program().stop();
                return;
            }
            WindowEvent::RedrawRequested => {
                self.inner.as_mut().unwrap().update(self.cursor);
                return;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let ch = keyboard::Key::Character(event.text.clone().unwrap_or_default());
                Event::Keyboard(match event.state {
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
                })
            }
            WindowEvent::MouseWheel { delta, .. } => Event::Mouse(mouse::Event::WheelScrolled {
                delta: match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => mouse::ScrollDelta::Lines {
                        x: x as f32,
                        y: y as f32,
                    },
                    winit::event::MouseScrollDelta::PixelDelta(physical_position) => {
                        mouse::ScrollDelta::Pixels {
                            x: physical_position.x as f32,
                            y: physical_position.y as f32,
                        }
                    }
                },
            }),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    winit::event::MouseButton::Left => mouse::Button::Left,
                    winit::event::MouseButton::Right => mouse::Button::Right,
                    winit::event::MouseButton::Middle => mouse::Button::Middle,
                    _ => mouse::Button::Other(99),
                };
                Event::Mouse(match state {
                    winit::event::ElementState::Pressed => mouse::Event::ButtonPressed(button),
                    winit::event::ElementState::Released => mouse::Event::ButtonReleased(button),
                })
            }
            _ => {
                return;
            }
        };
        let inner = self.inner.as_mut().unwrap();
        inner.program.queue_event(new_ev);
        inner.w.request_redraw();
    }
}

pub fn set_cursor(w: &Window, interaction: Interaction) {
    w.set_cursor(winit::window::Cursor::Icon(match interaction {
        iced_widget::core::mouse::Interaction::Pointer => winit::window::CursorIcon::Pointer,
        iced_widget::core::mouse::Interaction::Grab => winit::window::CursorIcon::Grabbing,
        _ => winit::window::CursorIcon::Default,
    }));
}

pub fn run<T: ProgramExt>(
    create_app: impl FnOnce(&(dyn (Fn() -> SendMsgFn<T::Message>) + Send + 'static)) -> T + 'static,
    options: WindowOptions,
) {
    let el = EventLoop::with_user_event().build().unwrap();
    let proxy = el.create_proxy();

    el.run_app(&mut App {
        inner: None,
        cursor: Cursor::Unavailable,
        prepared_app: Some((
            create_app(&move || {
                let proxy = proxy.clone();
                Box::new(move |msg: T::Message| {
                    if let Err(e) = proxy.send_event(msg) {
                        eprintln!("Error sending event: {}", e);
                    }
                })
            }),
            options,
        )),
    })
    .unwrap();
}
