#![feature(lazy_cell)]
#![feature(const_fn_floating_point_arithmetic)]

mod status;

use gdk::{
    cairo::{self, Context},
    glib::timeout_add_seconds_local,
};
use gtk::{prelude::*, ApplicationWindow, DrawingArea};
use gtk_layer_shell::{Edge, Layer, LayerShell};

/// Update interval in seconds.
const REFRESH_RATE: u32 = 2;

/// Number of bars and their thickness.
const N_BARS: i32 = 3;
const BAR_THICKNESS: i32 = 2;
const BAR_HEIGHT: i32 = 16;

const WIN_HEIGHT: i32 = BAR_HEIGHT;
const WIN_WIDTH: i32 = N_BARS * BAR_THICKNESS;

fn setup(app: &gtk::Application) {
    let win = ApplicationWindow::builder()
        .application(app)
        .default_width(WIN_WIDTH)
        .default_height(WIN_HEIGHT)
        .border_width(0)
        .app_paintable(true)
        .decorated(false)
        .title("sema")
        .build();

    // Set to be a layer surface
    win.init_layer_shell();
    win.set_layer(Layer::Overlay);

    // Anchor to bottom-right
    win.set_anchor(Edge::Right, true);
    win.set_anchor(Edge::Bottom, true);
    win.set_layer_shell_margin(Edge::Right, 2);
    win.set_layer_shell_margin(Edge::Bottom, 2);

    // Drawing the bars
    let drawing_area = DrawingArea::new();
    win.set_child(Some(&drawing_area));
    drawing_area.set_size_request(WIN_WIDTH, WIN_HEIGHT);
    drawing_area.connect_draw(|_, cr| {
        draw(cr);
        gtk::glib::Propagation::Stop
    });

    timeout_add_seconds_local(REFRESH_RATE, move || {
        drawing_area.queue_draw();
        gdk::glib::ControlFlow::Continue
    });

    win.show_all();
}

fn draw(cr: &Context) {
    // Transparent background
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    cr.set_operator(cairo::Operator::Source);
    cr.paint().expect("Failed to paint");

    // Draw the bars
    draw_bar(
        cr,
        2,
        0.0,
        status::battery().expect("Failed to get battery info"),
    );
    draw_bar(cr, 1, 0.0, status::volume());

    draw_bar(cr, 0, 0.8, (0.2, status::mic()));
    draw_bar(cr, 0, 0.6, (0.2, status::bluetooth()));
    draw_bar(cr, 0, 0.0, (0.5, status::wifi()));
}

/// Draw a single bar.
///
/// * `col`: column to draw the bar in. Automatically adjusts for bar width.
/// * `y`: y position to draw in as a percent of the window height.
/// * `percent`: height of the bar as a percent of the window height.
/// * `[r, g, b, a]`: decimal color to fill the bar with.
fn draw_bar(cr: &Context, col: i32, y: f64, (percent, [r, g, b, a]): (f64, [f64; 4])) {
    let filled = (WIN_HEIGHT as f64 * percent.min(1.)).floor();
    cr.rectangle(
        (col * BAR_THICKNESS) as f64,
        (1. - y) * WIN_HEIGHT as f64 - filled,
        BAR_THICKNESS as f64 - 0.5, // Take off a bit for spacing
        filled,
    );
    cr.set_source_rgba(r, g, b, a);
    cr.fill().expect("Failed to fill the bar");
}

fn main() {
    let application = gtk::Application::builder()
        .application_id("anarres.utils.sema")
        .build();

    application.connect_activate(setup);
    application.run();
}
