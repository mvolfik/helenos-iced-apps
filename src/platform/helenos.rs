use std::{
    ffi, mem,
    num::NonZero,
    ptr::NonNull,
    sync::{Arc, Condvar, Mutex},
};

use helenos_ui::util::pointer_init;
use iced_widget::core::{
    Event, Point,
    mouse::{self, Cursor, Interaction},
};
use raw_window_handle::{
    DisplayHandle, HasDisplayHandle, HasWindowHandle, HelenOSDisplayHandle, HelenOSWindowHandle,
    RawDisplayHandle, RawWindowHandle, WindowHandle,
};

use crate::AppInner;

#[derive(Debug)]
pub struct Window {
    raw: NonNull<helenos_ui::ui_window_t>,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, raw_window_handle::HandleError> {
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::HelenOS(HelenOSWindowHandle::new(
                self.raw.cast(),
            )))
        })
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, raw_window_handle::HandleError> {
        Ok(unsafe {
            DisplayHandle::borrow_raw(RawDisplayHandle::HelenOS(HelenOSDisplayHandle::new(
                NonNull::new(helenos_ui::ui_window_get_ui(self.raw.as_ptr()))
                    .unwrap()
                    .cast(),
            )))
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Window {
    pub fn inner_size(&self) -> Size {
        let rect = pointer_init(|ptr| unsafe {
            helenos_ui::ui_window_get_app_rect(self.raw.as_ptr(), ptr)
        })
        .unwrap();
        Size {
            width: (rect.p1.x - rect.p0.x) as u32,
            height: (rect.p1.y - rect.p0.y) as u32,
        }
    }

    pub fn scale_factor(&self) -> f64 {
        1.0
    }
}

pub fn set_cursor(w: &Window, interaction: Interaction) {}

static CONTROL_CALLBACKS: helenos_ui::ui_control_ops_t = helenos_ui::ui_control_ops_t {
    destroy: None,
    paint: Some(paint_event),
    kbd_event: None,
    pos_event: None,
    unfocus: None,
};

unsafe extern "C" fn paint_event(app: *mut ffi::c_void) -> i32 {
    println!("paint_event");
    let (app, _) = unsafe { &*(app as *const Arg) };
    let app = &mut *app.lock().unwrap();
    app.paint();
    println!("paint_event done");
    0 // EOK
}

static CALLBACKS: helenos_ui::ui_window_cb_t = helenos_ui::ui_window_cb_t {
    sysmenu: None,
    minimize: None,
    maximize: None,
    unmaximize: None,
    resize: Some(resize_event),
    close: Some(close_event),
    focus: None,
    kbd: None,
    paint: None,
    pos: Some(pos_event),
    unfocus: None,
};

type Arg = (Mutex<App>, Condvar);

unsafe extern "C" fn pos_event(
    _window: *mut helenos_ui::ui_window_t,
    app: *mut ffi::c_void,
    ev: *mut helenos_ui::pos_event_t,
) {
    type Evt = helenos_ui::pos_event_type_t;
    let (app, _) = unsafe { &*(app as *const Arg) };
    let ev = unsafe { &*ev };
    let mut app = app.lock().unwrap();
    let ev = match ev.type_ {
        Evt::POS_UPDATE => {
            let p = Point {
                x: ev.hpos as f32,
                y: ev.vpos as f32,
            };
            app.cursor = Cursor::Available(p);
            mouse::Event::CursorMoved { position: p }
        }
        Evt::POS_PRESS => {
            println!("Mouse pressed {:?}", ev);
            mouse::Event::ButtonPressed(mouse::Button::Left)
        }
        Evt::POS_RELEASE => mouse::Event::ButtonReleased(mouse::Button::Left),
        t => {
            println!("Unknown event type: {:?}", t);
            return;
        }
    };
    app.events_cache.push(Event::Mouse(ev));
}

unsafe extern "C" fn close_event(_window: *mut helenos_ui::ui_window_t, app: *mut ffi::c_void) {
    println!("close_event");
    let (app, condvar) = unsafe { &*(app as *const Arg) };
    let app = &mut *app.lock().unwrap();
    app.quit = true;
    condvar.notify_all();
}

unsafe extern "C" fn resize_event(window: *mut helenos_ui::ui_window_t, app: *mut ffi::c_void) {
    let (app, _) = unsafe { &*(app as *const Arg) };
    let mut app = app.lock().unwrap();
    let size = app.window.inner_size();
    app.surface
        .resize(
            NonZero::new(size.width).unwrap(),
            NonZero::new(size.height).unwrap(),
        )
        .unwrap();
    drop(app); // drop the lock before painting
    unsafe { helenos_ui::ui_window_paint(window) };
}

struct App {
    // inner: AppInner,
    quit: bool,
    window: Arc<Window>,
    _pin: std::marker::PhantomPinned,

    cursor: Cursor,
    events_cache: Vec<Event>,
    surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("quit", &self.quit)
            .field("window", &self.window)
            .finish_non_exhaustive()
    }
}

impl App {
    fn paint(&mut self) {
        let size = self.window.inner_size();
        println!("Painting... to {size:?}");
        // self.surface
        //     .resize(
        //         NonZero::new(size.width).unwrap(),
        //         NonZero::new(size.height).unwrap(),
        //     )
        //     .unwrap();
        let mut buffer = self.surface.buffer_mut().unwrap();
        for y in 0..size.height {
            for x in 0..size.width {
                let red = x % 255;
                let green = y % 255;
                let blue = (x * y) % 255;

                buffer[y as usize * size.width as usize + x as usize] =
                    blue | (green << 8) | (red << 16);
            }
        }

        println!("render done");
        buffer.present().unwrap();
        unsafe { helenos_ui::gfx_update(helenos_ui::ui_window_get_gc(self.window.raw.as_ptr())) };
        println!("paint done");
    }
}

pub fn main() {
    unsafe {
        let ui = pointer_init(|ptr| {
            helenos_ui::ui_create(helenos_ui::UI_DISPLAY_DEFAULT.as_ptr() as *const _, ptr)
        })
        .expect("Failed to open display");
        let init_w = 300;
        let init_h = 400;

        let mut wndparams = pointer_init(|ptr| helenos_ui::ui_wnd_params_init(ptr)).unwrap();
        wndparams.style |= helenos_ui::ui_wdecor_style_t::ui_wds_resizable
            | helenos_ui::ui_wdecor_style_t::ui_wds_maximize_btn;

        // helenos boilerplate code to create a window rectangle
        let mut rect1 = helenos_ui::gfx_rect_t {
            p0: helenos_ui::gfx_coord2_t { x: 0, y: 0 },
            p1: helenos_ui::gfx_coord2_t {
                x: init_w as i32,
                y: init_h as i32,
            },
        };
        let mut rect2 = pointer_init(|ptr| {
            helenos_ui::ui_wdecor_rect_from_app(ui, wndparams.style, &mut rect1, ptr)
        })
        .unwrap();
        let mut offset = rect2.p0;
        helenos_ui::gfx_rect_rtranslate(&mut offset, &mut rect2, &mut wndparams.rect);

        wndparams.min_size.x = 100;
        wndparams.min_size.y = 100;
        wndparams.caption = c"Iced Application".as_ptr();

        let window = pointer_init(|ptr| helenos_ui::ui_window_create(ui, &mut wndparams, ptr))
            .expect("Failed to create window");

        println!(
            "windor rect: {:?}",
            pointer_init(|ptr| { helenos_ui::ui_window_get_app_rect(window, ptr) }).unwrap()
        );

        run_app_in_window(Window {
            raw: NonNull::new(window).unwrap(),
        });
        helenos_ui::ui_destroy(ui);
    }
}

fn run_app_in_window(window: Window) {
    let arc = Arc::new(window);
    let app = std::pin::pin!((
        Mutex::new(App {
            window: arc.clone(),
            // inner: AppInner::new(arc),
            quit: false,
            _pin: std::marker::PhantomPinned,

            cursor: Cursor::Unavailable,
            events_cache: Vec::new(),
            surface: softbuffer::Surface::new(
                &softbuffer::Context::new(arc.clone()).unwrap(),
                arc.clone(),
            )
            .unwrap(),
        }),
        Condvar::new(),
    ));
    let app = app.into_ref();

    let ctl = pointer_init(|p| unsafe {
        helenos_ui::ui_control_new(
            &CONTROL_CALLBACKS as *const helenos_ui::ui_control_ops_t
                as *mut helenos_ui::ui_control_ops_t,
            app.as_ref().get_ref() as *const Arg as *mut ffi::c_void,
            p,
        )
    })
    .unwrap();
    unsafe {
        helenos_ui::ui_window_add(arc.raw.as_ptr(), ctl);
        helenos_ui::ui_window_set_cb(
            arc.raw.as_ptr(),
            &CALLBACKS as *const helenos_ui::ui_window_cb_t as *mut helenos_ui::ui_window_cb_t,
            app.as_ref().get_ref() as *const Arg as *mut ffi::c_void,
        );
        helenos_ui::ui_window_paint(arc.raw.as_ptr());
    }

    let _guard = app
        .1
        .wait_while(app.0.lock().unwrap(), |app| !app.quit)
        .unwrap();

    println!("Window closed, quitting...");

    unsafe { helenos_ui::ui_window_destroy(arc.raw.as_ptr()) };
}
