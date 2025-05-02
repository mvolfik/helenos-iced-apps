use std::{
    ffi, mem,
    ptr::NonNull,
    sync::{Arc, Mutex},
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

pub fn set_cursor(w: &Window, interaction: Interaction) {
    let curs = match interaction {
        Interaction::Pointer => helenos_ui::ui_stock_cursor_t::ui_curs_pointer,
        Interaction::Text => helenos_ui::ui_stock_cursor_t::ui_curs_ibeam,
        _ => helenos_ui::ui_stock_cursor_t::ui_curs_arrow,
    };
    unsafe { helenos_ui::ui_window_set_ctl_cursor(w.raw.as_ptr(), curs) };
}

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

type Arg = Mutex<App>;

unsafe extern "C" fn pos_event(
    window: *mut helenos_ui::ui_window_t,
    app: *mut ffi::c_void,
    ev: *mut helenos_ui::pos_event_t,
) {
    type Evt = helenos_ui::pos_event_type_t;
    let app = unsafe { &*(app as *const Arg) };
    let ev = unsafe { &*ev };
    let mut app = app.lock().unwrap();
    let ev = match ev.type_ {
        Evt::POS_UPDATE => {
            let app_rect = pointer_init(|p|unsafe {helenos_ui::ui_window_get_app_rect(window, p)}).unwrap();
            let p = Point {
                x: (ev.hpos as i32 - app_rect.p0.x) as f32,
                y: (ev.vpos as i32 - app_rect.p0.y) as f32,
            };
            app.cursor = Cursor::Available(p);
            mouse::Event::CursorMoved { position: p }
        }
        Evt::POS_PRESS => mouse::Event::ButtonPressed(mouse::Button::Left),
        Evt::POS_RELEASE => mouse::Event::ButtonReleased(mouse::Button::Left),
        _ => {
            return;
        }
    };
    app.events_cache.push(Event::Mouse(ev));
}

unsafe extern "C" fn close_event(_window: *mut helenos_ui::ui_window_t, app: *mut ffi::c_void) {
    let app = unsafe { &*(app as *const Arg) };
    let app = &mut *app.lock().unwrap();
    app.quit = true;
}

unsafe extern "C" fn paint_event(
    _window: *mut helenos_ui::ui_window_t,
    app: *mut ffi::c_void,
) -> i32 {
    let app = unsafe { &*(app as *const Arg) };
    let app = &mut *app.lock().unwrap();
    app.paint();
    0 // EOK
}

struct App {
    inner: AppInner,
    quit: bool,
    window: Arc<Window>,
    _pin: std::marker::PhantomPinned,

    cursor: Cursor,
    events_cache: Vec<Event>,
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
        self.inner
            .update(self.cursor, mem::take(&mut self.events_cache));
        unsafe { helenos_ui::gfx_update(helenos_ui::ui_window_get_gc(self.window.raw.as_ptr())) };
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
        wndparams.caption = c"Image viewer.rs".as_ptr();

        let window = pointer_init(|ptr| helenos_ui::ui_window_create(ui, &mut wndparams, ptr))
            .expect("Failed to create window");

        run_app_in_window(Window {
            raw: NonNull::new(window).unwrap(),
        });
        helenos_ui::ui_destroy(ui);
    }
}

fn run_app_in_window(window: Window) {
    let arc = Arc::new(window);
    let app = std::pin::pin!(Mutex::new(App {
        window: arc.clone(),
        inner: AppInner::new(arc.clone()),
        quit: false,
        _pin: std::marker::PhantomPinned,

        cursor: Cursor::Unavailable,
        events_cache: Vec::new(),
    }),);
    let app = app.into_ref();

    unsafe {
        helenos_ui::ui_window_set_cb(
            arc.raw.as_ptr(),
            &CALLBACKS as *const helenos_ui::ui_window_cb_t as *mut helenos_ui::ui_window_cb_t,
            app.as_ref().get_ref() as *const Arg as *mut ffi::c_void,
        );
        helenos_ui::ui_window_paint(arc.raw.as_ptr());
    }

    loop {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let app = app.lock().unwrap();
        if app.quit {
            break;
        }
        if !app.events_cache.is_empty() {
            // drop the lock, process events and repaint
            drop(app);
            unsafe {
                helenos_ui::ui_window_paint(arc.raw.as_ptr());
            }
        }
    }

    println!("Window closed, quitting...");

    unsafe { helenos_ui::ui_window_destroy(arc.raw.as_ptr()) };
}
