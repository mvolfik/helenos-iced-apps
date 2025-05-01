use std::{
    ffi, mem,
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
    ui: NonNull<helenos_ui::ui_t>,
    window: NonNull<helenos_ui::ui_window_t>,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, raw_window_handle::HandleError> {
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::HelenOS(HelenOSWindowHandle::new(
                self.window.cast(),
            )))
        })
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, raw_window_handle::HandleError> {
        Ok(unsafe {
            DisplayHandle::borrow_raw(RawDisplayHandle::HelenOS(HelenOSDisplayHandle::new(
                self.ui.cast(),
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
            helenos_ui::ui_window_get_app_rect(self.window.as_ptr(), ptr)
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

static CALLBACKS: helenos_ui::ui_window_cb_t = helenos_ui::ui_window_cb_t {
    sysmenu: None,
    minimize: None,
    maximize: None,
    unmaximize: None,
    resize: None,
    close: Some(close_event),
    focus: None,
    kbd: None,
    paint: Some(paint_event),
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
    let (app, condvar) = unsafe { &*(app as *const Arg) };
    let mut app = app.lock().unwrap();
    app.quit = true;
    condvar.notify_all();
}

unsafe extern "C" fn paint_event(_window: *mut helenos_ui::ui_window_t, app: *mut ffi::c_void) -> i32 {
    let (app, _) = unsafe { &*(app as *const Arg) };
    let mut app = &mut *app.lock().unwrap();
    app.inner
        .update(app.cursor, mem::take(&mut app.events_cache));
    0 // EOK
}

#[derive(Debug)]
struct App {
    inner: AppInner,
    quit: bool,
    window: Arc<Window>,
    _pin: std::marker::PhantomPinned,

    cursor: Cursor,
    events_cache: Vec<Event>,
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

        let arc = Arc::new(Window {
            ui: NonNull::new(ui).unwrap(),
            window: NonNull::new(window).unwrap(),
        });
        let app = std::pin::pin!((
            Mutex::new(App {
                window: arc.clone(),
                inner: AppInner::new(arc),
                quit: false,
                _pin: std::marker::PhantomPinned,

                cursor: Cursor::Unavailable,
                events_cache: Vec::new(),
            }),
            Condvar::new()
        ));
        let app = app.into_ref();
        helenos_ui::ui_window_set_cb(
            window,
            &CALLBACKS as *const helenos_ui::ui_window_cb_t as *mut helenos_ui::ui_window_cb_t,
            app.as_ref().get_ref() as *const Arg as *mut ffi::c_void,
        );

        let mut guard = app
            .1
            .wait_while(app.0.lock().unwrap(), |app| !app.quit)
            .unwrap();

        println!("Window closed, quitting...");

        unsafe { helenos_ui::ui_window_destroy(window) };
        let _ = (guard, app);
        unsafe { helenos_ui::ui_destroy(ui) };
    }
}
