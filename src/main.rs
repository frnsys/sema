mod bars;
mod status;

use bars::{Bar, BAR_THICKNESS};
use gdk::{
    cairo::{self, Context},
    glib::timeout_add_seconds_local,
};
use gtk::{prelude::*, ApplicationWindow, DrawingArea};
use gtk_layer_shell::{Edge, Layer, LayerShell};
use status::Status;

/// Update interval in seconds.
const REFRESH_RATE: u32 = 2;

const WIN_HEIGHT: i32 = BAR_THICKNESS;

const SCHEMA: &[Bar] = &[
    Bar {
        width: 4,
        status: Status::Wifi,
    },
    Bar {
        width: 4,
        status: Status::Bluetooth,
    },
    Bar {
        width: 12,
        status: Status::Volume,
    },
    Bar {
        width: 12,
        status: Status::Battery,
    },
];

fn setup(app: &gtk::Application) {
    let win_width = SCHEMA.iter().map(|bar| bar.width).sum::<u32>() as i32;

    let win = ApplicationWindow::builder()
        .application(app)
        .default_width(win_width)
        .default_height(WIN_HEIGHT)
        .border_width(0)
        .app_paintable(true)
        .decorated(false)
        .resizable(false)
        .can_focus(false)
        .has_tooltip(false)
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
    drawing_area.set_size_request(win_width, WIN_HEIGHT);
    drawing_area.connect_draw(|_, ctx| {
        if let Err(err) = draw(ctx) {
            eprintln!("{}", err);
        }
        gtk::glib::Propagation::Stop
    });

    timeout_add_seconds_local(REFRESH_RATE, move || {
        drawing_area.queue_draw();
        gdk::glib::ControlFlow::Continue
    });

    win.show_all();
}

fn draw(ctx: &Context) -> Result<(), String> {
    // Transparent background
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    ctx.set_operator(cairo::Operator::Source);
    ctx.paint().expect("Failed to paint");

    // Draw the bars
    let mut x = 0.;
    for bar in SCHEMA {
        x = bar.draw(ctx, x)?;
    }
    Ok(())
}

fn main() {
    let application = gtk::Application::builder()
        .application_id("anarres.utils.sema")
        .build();

    application.connect_activate(setup);
    application.run();
}
