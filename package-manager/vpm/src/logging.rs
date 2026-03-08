use std::io::{self, IsTerminal};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};

use crate::cli::ColorMode;

static VPM_VERBOSE: AtomicBool = AtomicBool::new(false);
static VPM_COLOR_MODE: AtomicU8 = AtomicU8::new(ColorMode::Auto as u8);

pub struct Loader {
    label: String,
    start: Instant,
}

impl Loader {
    pub fn start(label: impl Into<String>) -> Self {
        let label = label.into();
        println!("{} {}", icon_loader(), label);
        Self {
            label,
            start: Instant::now(),
        }
    }

    pub fn done(self) {
        let elapsed = format_elapsed(self.start.elapsed());
        println!("{} {} {}", icon_success(), self.label, muted(&format!("({elapsed})")));
    }
}

pub fn set_cli_runtime_config(verbose: bool, color: ColorMode) {
    VPM_VERBOSE.store(verbose, Ordering::Relaxed);
    VPM_COLOR_MODE.store(color as u8, Ordering::Relaxed);
}

fn verbose_enabled() -> bool {
    VPM_VERBOSE.load(Ordering::Relaxed)
}

fn color_mode() -> ColorMode {
    match VPM_COLOR_MODE.load(Ordering::Relaxed) {
        1 => ColorMode::Always,
        2 => ColorMode::Never,
        _ => ColorMode::Auto,
    }
}

fn use_color(stderr: bool) -> bool {
    match color_mode() {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            if std::env::var_os("NO_COLOR").is_some() {
                return false;
            }
            if matches!(std::env::var("CLICOLOR_FORCE").ok().as_deref(), Some("1")) {
                return true;
            }
            if matches!(std::env::var("CLICOLOR").ok().as_deref(), Some("0")) {
                return false;
            }
            if stderr {
                io::stderr().is_terminal()
            } else {
                io::stdout().is_terminal()
            }
        }
    }
}

pub(crate) fn paint(text: &str, ansi_code: &str, stderr: bool) -> String {
    if use_color(stderr) {
        format!("\x1b[{ansi_code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub(crate) fn muted(text: &str) -> String {
    paint(text, "2", false)
}

fn icon_success() -> String {
    paint("✓", "1;32", false)
}

fn icon_info() -> String {
    paint("ℹ", "1;36", false)
}

fn icon_warn() -> String {
    paint("⚠", "1;33", false)
}

fn icon_error() -> String {
    paint("✗", "1;31", true)
}

fn icon_loader() -> String {
    paint("⟳", "1;34", false)
}

pub fn log_success(message: &str) {
    println!("{} {}", icon_success(), message);
}

pub fn log_info(message: &str) {
    println!("{} {}", icon_info(), message);
}

pub fn log_warn(message: &str) {
    println!("{} {}", icon_warn(), message);
}

pub fn log_error(message: &str) {
    eprintln!("{} {}", icon_error(), paint(message, "31", true));
}

pub fn log_verbose(message: &str) {
    if verbose_enabled() {
        println!("{} {}", paint("›", "2;36", false), paint(message, "2;36", false));
    }
}

fn format_elapsed(duration: Duration) -> String {
    if duration.as_secs_f64() < 1.0 {
        format!("{}ms", duration.as_millis())
    } else {
        format!("{:.2}s", duration.as_secs_f64())
    }
}
