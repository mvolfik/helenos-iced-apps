use iced_widget::graphics::color;
use xcb::{
    dri3,
    x::{self, Screen},
};

fn main() {
    umain().unwrap();
}

enum Step {
    IncreaseG,
    DecreaseR,
    IncreaseB,
    DecreaseG,
    IncreaseR,
    DecreaseB,
}

struct ColorIt {
    r: u8,
    g: u8,
    b: u8,
    step: Step,
}

impl Iterator for ColorIt {
    type Item = (u8, u8, u8);

    fn next(&mut self) -> Option<Self::Item> {
        use Step::*;
        let (v, incr, next) = match self.step {
            IncreaseG => (&mut self.g, true, DecreaseR),
            DecreaseR => (&mut self.r, false, IncreaseB),
            IncreaseB => (&mut self.b, true, DecreaseG),
            DecreaseG => (&mut self.g, false, IncreaseR),
            IncreaseR => (&mut self.r, true, DecreaseB),
            DecreaseB => (&mut self.b, false, IncreaseG),
        };
        if incr {
            *v += 1
        } else {
            *v -= 1
        }
        if *v == 0 || *v == 255 {
            self.step = next;
        }
        Some((self.r, self.g, self.b))
    }
}

fn umain() -> anyhow::Result<()> {
    let (conn, screen_num) = xcb::Connection::connect(None)?;

    // Fetch the `x::Setup` and get the main `x::Screen` object.
    let setup = conn.get_setup();
    let screen = setup.roots().nth(screen_num as usize).unwrap();

    // Generate an `Xid` for the client window.
    // The type inference is needed here.
    let window: x::Window = conn.generate_id();

    let S: usize = 500;

    // We can now create a window. For this we pass a `Request`
    // object to the `send_request_checked` method. The method
    // returns a cookie that will be used to check for success.
    conn.send_and_check_request(&x::CreateWindow {
        depth: x::COPY_FROM_PARENT as u8,
        wid: window,
        parent: screen.root(),
        x: 0,
        y: 0,
        width: S as u16,
        height: S as u16,
        border_width: 0,
        class: x::WindowClass::InputOutput,
        visual: screen.root_visual(),
        // this list must be in same order than `Cw` enum order
        value_list: &[
            x::Cw::BackPixel(screen.white_pixel()),
            x::Cw::EventMask(x::EventMask::EXPOSURE | x::EventMask::KEY_PRESS),
        ],
    })?;
    conn.send_and_check_request(&x::MapWindow { window })?;

    let gc = conn.generate_id();
    conn.send_and_check_request(&x::CreateGc {
        cid: gc,
        drawable: x::Drawable::Window(window),
        value_list: &[x::Gc::Foreground(screen.black_pixel())],
    })?;

    let pixmap = conn.generate_id();
    conn.send_and_check_request(&x::CreatePixmap {
        depth: 24,
        pid: pixmap,
        drawable: x::Drawable::Window(window),
        width: S as u16,
        height: S as u16,
    })?;

    let resp = conn.wait_for_reply(conn.send_request(&dri3::BufferFromPixmap { pixmap }))?;
    let len = resp.size() as usize;
    let stride = resp.stride() as usize;
    println!("Buffer length: {len} stride {stride} {resp:?}");
    
    assert!(resp.bpp() == 32);
    assert!(resp.depth() == 24);

    let mut map = unsafe {
        memmap2::MmapOptions::new()
            .len(len)
            .map_mut(resp.pixmap_fd())
    }?;

    conn.flush()?;


    let mut colors = ColorIt {
        r: 255,
        g: 0,
        b: 0,
        step: Step::IncreaseG,
    };

    // for y in 0..S {
    //     for x in 0..S {
    //         let bi = y * stride + x * 3;
    //         if y > 100 && y < 150 && x > 300 && x < 400 {
    //             map[bi] = 255;
    //             map[bi + 1] = 0;
    //             map[bi + 2] = 0;
    //             map[bi + 3] = 0;
    //         } else {
    //             map[bi] = 0;
    //             map[bi + 1] = 0;
    //             map[bi + 2] = 0;
    //             map[bi + 3] = 0;
    //         }
    //     }
    // }

    for i in 0..len {
        map[i] = 0;
    }

    for y in 0..S {
        for x in [25] {
            let bi = y * stride + x * 4;
            map[bi] = y as u8;
            map[bi + 1] = 0;
            map[bi + 2] = 0;
            map[bi + 3] = 0;
        }
    }
    map.flush()?;

    conn.send_and_check_request(&x::CopyArea {
        src_drawable: x::Drawable::Pixmap(pixmap),
        dst_drawable: x::Drawable::Window(window),
        gc,
        src_x: 0,
        src_y: 0,
        dst_x: 0,
        dst_y: 0,
        width: S as u16,
        height: S as u16,
    })?;

    conn.flush()?;

    loop {
        match conn.wait_for_event()? {
            xcb::Event::X(x::Event::KeyPress(ev)) => {
            }
            _ => {}
        }
    }
    Ok(())
}

fn draw_black(
    conn: &xcb::Connection,
    screen: &Screen,
    window: x::Window,
) -> Result<(), xcb::Error> {
    let cid = conn.generate_id();
    conn.send_and_check_request(&x::CreateGc {
        cid,
        drawable: x::Drawable::Window(window),
        value_list: &[x::Gc::Foreground(screen.black_pixel())],
    })?;

    // conn.send_and_check_request(&x::CreatePixmap {
    //     depth: 24,
    //     pid: pxid,
    //     drawable: x::Drawable::Window(window),
    //     width: 150,
    //     height: 150,
    // })?;
    conn.flush()?;

    let mut x = 0;
    let mut y = 0;
    loop {
        match conn.wait_for_event()? {
            xcb::Event::X(x::Event::KeyPress(ev)) => {
                println!("KeyPress: {:?}", ev);
                for i in 0..20 {
                    conn.send_request_checked(&x::PolyPoint {
                        coordinate_mode: x::CoordMode::Origin,
                        drawable: x::Drawable::Window(window),
                        gc: cid,
                        points: &[x::Point { x, y }],
                    });
                    x += 1;
                    if x > 100 {
                        x = 0;
                        y += 1;
                    }
                }
                conn.flush()?;
            }
            _ => {}
        }
    }
}
