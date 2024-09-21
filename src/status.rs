use std::{process::Command, sync::LazyLock};

use regex_lite::Regex;

use crate::bars::{rgba, BarFill, Rgba};

const COLOR_URGENT: Rgba = rgba(0xcf4955ff);
const COLOR_WARN: Rgba = rgba(0xfbc011ff);
const COLOR_OK: Rgba = rgba(0x0a8c6cff);
const COLOR_BG: Rgba = rgba(0x161616ff);
const COLOR_MUTE: Rgba = rgba(0x777777ff);
const COLOR_NORMAL: Rgba = rgba(0x256ccfff);
const COLOR_VOLUME: Rgba = rgba(0xccccccff);

pub enum Status {
    Volume,
    Wifi,
    Battery,
    Bluetooth,
}
impl Status {
    pub fn fill(&self) -> Result<BarFill, String> {
        match self {
            Self::Volume => volume(),
            Self::Wifi => wifi(),
            Self::Battery => battery().map_err(|_| "Failed to get battery info".into()),
            Self::Bluetooth => bluetooth(),
        }
    }
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
fn battery() -> Result<BarFill, battery::Error> {
    let manager = battery::Manager::new()?;
    let batt = manager
        .batteries()?
        .next()
        .expect("Should be at least one battery")?;
    let bar = match batt.state() {
        // "Not Charging" state not yet supported,
        // reported as "Unknown".
        // https://github.com/svartalf/rust-battery/pull/100
        battery::State::Unknown => (1.0, COLOR_OK),
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
    Ok(BarFill {
        width: bar.0,
        color: bar.1,
    })
}

/// Get a bar representing the volume state.
fn volume() -> Result<BarFill, String> {
    static PERCENT_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(\d{1,3})%"#).expect("Should be a valid regex"));

    let out = cmd("pactl", &["--", "get-sink-mute", "@DEFAULT_SINK@"])?;
    let muted = out.contains("yes");
    let fill_color = if muted { COLOR_MUTE } else { COLOR_VOLUME };

    let out = cmd("pactl", &["--", "get-sink-volume", "@DEFAULT_SINK@"])?;
    let caps = PERCENT_RE.captures(&out).expect("Volume should be present");
    let volume: f64 = caps
        .get(1)
        .expect("Volume should be present")
        .as_str()
        .parse()
        .expect("Volume should be valid number");
    Ok(BarFill {
        width: volume / 100.,
        color: fill_color,
    })
}

/// Get a bar representing the bluetooth state.
fn bluetooth() -> Result<BarFill, String> {
    let out = cmd("bluetoothctl", &["show"])?;
    let color = if out.contains("Powered: yes") {
        COLOR_NORMAL
    } else {
        COLOR_BG
    };
    Ok(BarFill { width: 1., color })
}

/// Get a color representing the wifi/vpn state.
fn wifi() -> Result<BarFill, String> {
    let out = cmd("ip", &["address"])?;
    let color = if !out.contains("state UP") {
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
    Ok(BarFill { width: 1., color })
}
