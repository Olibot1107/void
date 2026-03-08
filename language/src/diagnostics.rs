use std::io::{self, IsTerminal};
use std::sync::OnceLock;

const ANSI_RED: &str = "\x1b[31m";
const ANSI_BOLD_RED: &str = "\x1b[1;31m";
const ANSI_RESET: &str = "\x1b[0m";

fn stderr_supports_color() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        if std::env::var_os("NO_COLOR").is_some() {
            return false;
        }

        if matches!(std::env::var("CLICOLOR_FORCE").ok().as_deref(), Some("1")) {
            return true;
        }

        if matches!(std::env::var("CLICOLOR").ok().as_deref(), Some("0")) {
            return false;
        }

        io::stderr().is_terminal()
    })
}

pub fn error_label(text: &str) -> String {
    if stderr_supports_color() {
        format!("{ANSI_BOLD_RED}{text}{ANSI_RESET}")
    } else {
        text.to_string()
    }
}

pub fn error_text(text: &str) -> String {
    if stderr_supports_color() {
        format!("{ANSI_RED}{text}{ANSI_RESET}")
    } else {
        text.to_string()
    }
}
