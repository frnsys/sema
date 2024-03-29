use std::{process::Command, sync::LazyLock};

use regex_lite::Regex;

const COLOR_URGENT: Rgba = rgba(0xcf4955ff);
const COLOR_WARN: Rgba = rgba(0xfbc011ff);
const COLOR_OK: Rgba = rgba(0x0a8c6cff);
const COLOR_BG: Rgba = rgba(0x161616ff);
const COLOR_MUTE: Rgba = rgba(0x777777ff);
const COLOR_NORMAL: Rgba = rgba(0x256ccfff);

type Rgba = [f64; 4];
type Bar = (f64, Rgba);

const fn rgba(color: u32) -> Rgba {
    let r = ((color >> 24) & 0xFF) as f64 / 255.0;
    let g = ((color >> 16) & 0xFF) as f64 / 255.0;
    let b = ((color >> 8) & 0xFF) as f64 / 255.0;
    let a = (color & 0xFF) as f64 / 255.0;
    [r, g, b, a]
}

/// Run a shell command and get the output.
fn cmd(cmd: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)
            .expect("Should be utf8")
            .trim()
            .to_string();
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let err = format!("Command {} failed, error: {}", cmd, stderr);
        Err(err.to_string())
    }
}

/// Get a bar representing the battery state.
pub fn battery() -> Result<Bar, battery::Error> {
    let manager = battery::Manager::new()?;
    let batt = manager
        .batteries()?
        .next()
        .expect("Should be at least one battery")?;
    let bar = match batt.state() {
        battery::State::Full => (1.0, COLOR_OK),
        battery::State::Charging => {
            let percent = batt.state_of_charge().value as f64;
            (percent, COLOR_OK)
        }
        battery::State::Discharging => {
            let percent = batt.state_of_charge().value as f64;
            let color = if percent <= 0.1 {
                COLOR_URGENT
            } else {
                COLOR_WARN
            };
            (percent, color)
        }
        _ => (1.0, COLOR_BG),
    };
    Ok(bar)
}

/// Get a bar representing the volume state.
pub fn volume() -> Result<Bar, String> {
    static PERCENT_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(\d{1,3})%"#).expect("Should be a valid regex"));

    let out = cmd("pactl", &["--", "get-sink-mute", "@DEFAULT_SINK@"])?;
    let muted = out.contains("yes");
    let fill_color = if muted { COLOR_MUTE } else { COLOR_NORMAL };

    let out = cmd("pactl", &["--", "get-sink-volume", "@DEFAULT_SINK@"])?;
    let caps = PERCENT_RE.captures(&out).expect("Volume should be present");
    let volume: f64 = caps
        .get(1)
        .expect("Volume should be present")
        .as_str()
        .parse()
        .expect("Volume should be valid number");
    Ok((volume / 100., fill_color))
}

/// Get a color representing the bluetooth state.
pub fn bluetooth() -> Result<Rgba, String> {
    let out = cmd("bt", &[])?;
    let color = if out == "on" { COLOR_NORMAL } else { COLOR_BG };
    Ok(color)
}

/// Get a color representing the microphone state.
pub fn mic() -> Result<Rgba, String> {
    let out = cmd("mute", &["status"])?;
    let color = if out == "yes" { COLOR_BG } else { COLOR_URGENT };
    Ok(color)
}

/// Get a color representing the wifi/vpn state.
pub fn wifi() -> Result<Rgba, String> {
    let out = cmd("/usr/bin/wifi", &[])?;
    let color = if !out.contains("on") {
        COLOR_BG
    } else {
        let out = cmd("mullvad", &["status"])?;
        let ssid = cmd("iwgetid", &["-r"]).unwrap_or("".into());
        if out.contains("Connected") {
            COLOR_OK
        } else if ssid.is_empty() {
            COLOR_MUTE
        } else {
            COLOR_URGENT
        }
    };
    Ok(color)
}
