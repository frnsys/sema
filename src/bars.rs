use gdk::cairo::Context;

use crate::status::Status;

pub const BAR_THICKNESS: i32 = 1;

pub struct Bar {
    pub width: u32,
    pub status: Status,
}
impl Bar {
    /// Draw a single bar.
    ///
    /// Returns the x-offset for the next bar.
    pub fn draw(&self, ctx: &Context, x: f64) -> Result<f64, String> {
        let fill = self.status.fill()?;
        let filled = (self.width as f64 * fill.width.min(1.)).floor();
        ctx.rectangle(
            x,
            0.,
            filled - 1., // Take off a bit for spacing
            BAR_THICKNESS as f64,
        );
        let [r, g, b, a] = fill.color;
        ctx.set_source_rgba(r, g, b, a);
        ctx.fill().expect("Failed to fill the bar");
        Ok(x + self.width as f64)
    }
}

pub type Rgba = [f64; 4];
pub const fn rgba(color: u32) -> Rgba {
    let r = ((color >> 24) & 0xFF) as f64 / 255.0;
    let g = ((color >> 16) & 0xFF) as f64 / 255.0;
    let b = ((color >> 8) & 0xFF) as f64 / 255.0;
    let a = (color & 0xFF) as f64 / 255.0;
    [r, g, b, a]
}

pub struct BarFill {
    /// Width in [0.0, 1.0];
    /// how much of the bar's allocated
    /// width should be filled.
    pub width: f64,

    /// Fill color for the bar.
    pub color: Rgba,
}
