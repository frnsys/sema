#![feature(lazy_cell)]

use std::{
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock,
    },
    thread,
    time::Duration,
};

use pixels::{
    wgpu::{BlendState, Color, CompositeAlphaMode},
    Error, Pixels, PixelsBuilder, SurfaceTexture,
};
use regex_lite::Regex;
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoopBuilder,
    window::{WindowBuilder, WindowLevel},
};

/// Update interval in seconds.
const REFRESH_RATE: u64 = 2;

/// Display scale factor.
const SCALE_FACTOR: usize = 2;

/// Number of bars and their thickness.
const N_BARS: usize = 3;
const BAR_THICKNESS: usize = 2;

/// Right and bottom margin around the window.
const MARGIN: usize = 2;

/// The dimensions of the inner contents (i.e. excluding the margins).
/// The bars will be represented in memory as rows,
/// but they will be rendered as columns (i.e. vertically).
const INNER_HEIGHT: usize = 16;
const INNER_WIDTH: usize = N_BARS * BAR_THICKNESS;

/// Window dimensions taking into account the r/b margin.
const WIN_HEIGHT: usize = INNER_HEIGHT + MARGIN;
const WIN_WIDTH: usize = INNER_WIDTH + MARGIN;

/// Dimensions of the bars in actual pixels
const BAR_LENGTH: usize = INNER_HEIGHT * SCALE_FACTOR;
const BAR_GIRTH: usize = BAR_THICKNESS * SCALE_FACTOR;

/// The real width in pixels (taking into account scale factor).
const REAL_WIDTH: usize = WIN_WIDTH * SCALE_FACTOR;

/// Single RGBA pixel.
type Rgba = [u8; 4];

/// A bar (sequence of pixels).
type Bar = [Rgba; BAR_LENGTH];

/// A sequence of bars (i.e. the canvas).
type Matrix = [Bar; REAL_WIDTH];

const COLOR_URGENT: Rgba = [0xcf, 0x49, 0x55, 0xff];
const COLOR_WARN: Rgba = [0xfb, 0xc0, 0x11, 0xff];
const COLOR_OK: Rgba = [0x0a, 0x8c, 0x6c, 0xff];
const COLOR_BG: Rgba = [0x16, 0x16, 0x16, 0xff];
const COLOR_MUTE: Rgba = [0x77, 0x77, 0x77, 0xff];
const COLOR_NORMAL: Rgba = [0x25, 0x6c, 0xcf, 0xff];

/// Run a shell command and get the output.
fn cmd(cmd: &str, args: &[&str]) -> String {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        String::from_utf8(output.stdout)
            .expect("Should be utf8")
            .trim()
            .to_string()
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("Command failed, error: {}", stderr);
    }
}

/// Draw a filled bar.
fn bar(percent: f32, fill_color: Rgba) -> Bar {
    let filled = (BAR_LENGTH as f32 * percent).floor() as usize;
    let mut bar = [fill_color; BAR_LENGTH];
    bar[filled..].fill(COLOR_BG);
    bar
}

/// Get a bar representing the battery state.
fn battery() -> Result<Bar, battery::Error> {
    let manager = battery::Manager::new()?;
    let batt = manager
        .batteries()?
        .next()
        .expect("Should be at least one battery")?;
    let pixels = match batt.state() {
        battery::State::Full => [COLOR_OK; BAR_LENGTH],
        battery::State::Charging => {
            let percent = batt.state_of_charge().value;
            bar(percent, COLOR_OK)
        }
        battery::State::Discharging => {
            let percent = batt.state_of_charge().value;
            let color = if percent <= 0.1 {
                COLOR_URGENT
            } else {
                COLOR_WARN
            };
            bar(percent, color)
        }
        _ => [COLOR_BG; BAR_LENGTH],
    };
    Ok(pixels)
}

/// Get a color representing the bluetooth state.
fn bluetooth() -> Rgba {
    let out = cmd("bt", &[]);
    if out == "on" {
        COLOR_NORMAL
    } else {
        COLOR_BG
    }
}

/// Get a bar representing the volume state.
fn volume() -> Bar {
    static PERCENT_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(\d{1,3})%"#).expect("Should be a valid regex"));

    let out = cmd("pactl", &["--", "get-sink-mute", "@DEFAULT_SINK@"]);
    let muted = out.contains("yes");
    let fill_color = if muted { COLOR_MUTE } else { COLOR_NORMAL };

    let out = cmd("pactl", &["--", "get-sink-volume", "@DEFAULT_SINK@"]);
    let caps = PERCENT_RE.captures(&out).expect("Volume should be present");
    let volume: f32 = caps
        .get(1)
        .expect("Volume should be present")
        .as_str()
        .parse()
        .expect("Volume should be valid number");
    bar(volume / 100., fill_color)
}

/// Get a color representing the microphone state.
fn mic() -> Rgba {
    let out = cmd("mute", &["status"]);
    if out == "yes" {
        COLOR_BG
    } else {
        COLOR_URGENT
    }
}

/// Get a color representing the wifi/vpn state.
fn wifi() -> Rgba {
    let out = cmd("/usr/bin/wifi", &[]);
    if !out.contains("on") {
        COLOR_BG
    } else {
        let out = cmd("mullvad", &["status"]);
        if out.contains("Connected") {
            COLOR_OK
        } else {
            COLOR_URGENT
        }
    }
}

/// Event to refresh the states.
#[derive(Debug)]
struct Refresh;

fn main() -> Result<(), Error> {
    let event_loop = EventLoopBuilder::<Refresh>::with_user_event()
        .build()
        .expect("Failed to create event loop");

    // Setup a background thread to indicate when to
    // update the matrix data.
    let done = Arc::new(AtomicBool::new(false));
    let should_exit = Arc::clone(&done);
    let proxy = event_loop.create_proxy();
    let handle = thread::spawn(move || {
        while !should_exit.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(REFRESH_RATE));
            if proxy.send_event(Refresh).is_err() {
                break;
            }
        }
    });

    // Initialize the matrix.
    let mut matrix: Matrix = [[[0; 4]; BAR_LENGTH]; REAL_WIDTH];
    refresh(&mut matrix);

    // Note: For Wayland positioning has to happen via the window manager.
    let window = WindowBuilder::new()
        .with_title("sema")
        .with_resizable(false)
        .with_decorations(false)
        .with_transparent(true)
        .with_window_level(WindowLevel::AlwaysOnTop)
        .with_inner_size(LogicalSize::new(WIN_WIDTH as f64, WIN_HEIGHT as f64))
        .build(&event_loop)
        .unwrap();

    let window_size = window.inner_size();
    let surface_texture = SurfaceTexture::new(
        window_size.width * SCALE_FACTOR as u32,
        window_size.height * SCALE_FACTOR as u32,
        &window,
    );
    let mut pixels = PixelsBuilder::new(
        (WIN_WIDTH * SCALE_FACTOR) as u32,
        (WIN_HEIGHT * SCALE_FACTOR) as u32,
        surface_texture,
    )
    .clear_color(Color::TRANSPARENT)
    .blend_state(BlendState::ALPHA_BLENDING)
    .alpha_mode(CompositeAlphaMode::PreMultiplied)
    .build()?;

    event_loop
        .run(move |event, target| match event {
            Event::UserEvent(Refresh) => {
                refresh(&mut matrix);
                draw(&matrix, &mut pixels);
                if pixels.render().is_err() {
                    target.exit();
                }
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                draw(&matrix, &mut pixels);
                if pixels.render().is_err() {
                    target.exit();
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => target.exit(),
            _ => (),
        })
        .expect("Event loop failed");

    done.store(true, Ordering::SeqCst);
    handle.join().unwrap();

    Ok(())
}

/// Update the matrix pixels.
fn refresh(mat: &mut Matrix) {
    // The first bar has multiple segments.
    let mut bar = [[0; 4]; BAR_LENGTH];

    let wifi_length = 10;
    bar[0..wifi_length].fill(wifi());

    let bt_length = 6;
    bar[(BAR_LENGTH - bt_length)..BAR_LENGTH].fill(bluetooth());

    let mic_length = 6;
    bar[(BAR_LENGTH - bt_length - mic_length - 1)..(BAR_LENGTH - bt_length - 1)].fill(mic());

    let bars = [
        bar,
        volume(),
        battery().expect("Failed to read battery info"),
    ];
    for (i, bar) in bars.into_iter().enumerate() {
        let x = i * BAR_GIRTH;
        mat[x..(x + BAR_GIRTH - 1)].fill(bar);
    }
}

/// Iterate over the matrix as if it were rotated 90deg CCW.
fn rot90ccw(mat: &Matrix) -> impl Iterator<Item = &Rgba> {
    let rows = mat.len();
    let cols = mat[0].len();

    (0..cols)
        .rev()
        .flat_map(move |col| (0..rows).map(move |row| &mat[row][col]))
}

/// Draw the (rotated) matrix to the screen.
fn draw(mat: &Matrix, pixels: &mut Pixels) {
    let frame: &mut [u8] = pixels.frame_mut();
    for (pixel, data) in frame.chunks_exact_mut(4).zip(rot90ccw(mat)) {
        pixel.copy_from_slice(data);
    }
}
