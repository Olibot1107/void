mod ast;
mod diagnostics;
mod lexer;
mod parser;
mod runtime;
mod value;

use std::io::{self, Write};
use std::path::Path;

use diagnostics::{error_label, error_text, info_label, info_text, success_text};
use runtime::Runtime;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    println!(
        "Void {VERSION}\nUsage:\n  void                 Start interactive REPL\n  void <file-or-dir> [args...]  Run script or directory entry\n\nOptions:\n  --help, -h       Show help\n  --version, -v    Show version"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let first = args.first().map(String::as_str);
    match first {
        None => {
            if let Err(err) = run_repl() {
                eprintln!("{}:\n{}", error_label("Void REPL error"), error_text(&err));
                std::process::exit(1);
            }
        }
        Some("--help") | Some("-h") => {
            print_help();
            return;
        }
        Some("--version") | Some("-v") => {
            println!("{VERSION}");
            return;
        }
        Some(path) => {
            let script_args = if args.len() > 1 {
                args[1..].to_vec()
            } else {
                Vec::new()
            };
            let mut runtime = Runtime::new(script_args);
            if let Err(err) = runtime.run_entry(Path::new(path)) {
                eprintln!("{}:\n{}", error_label("Void runtime error"), error_text(&err));
                std::process::exit(1);
            }
        }
    }
}

fn run_repl() -> Result<(), String> {
    let mut runtime = Runtime::new(Vec::new());
    let stdin = io::stdin();
    let mut buffer = String::new();

    println!("{}", info_label(&format!("Welcome to Void v{VERSION}")));
    println!("{}", info_text("Type .help for help, .exit to quit."));

    loop {
        let prompt = if buffer.is_empty() { "> " } else { "... " };
        print!("{}", success_text(prompt));
        io::stdout().flush().map_err(|e| e.to_string())?;

        let mut line = String::new();
        let bytes = stdin.read_line(&mut line).map_err(|e| e.to_string())?;
        if bytes == 0 {
            println!();
            break;
        }

        let trimmed = line.trim();
        if buffer.is_empty() {
            match trimmed {
                "" => continue,
                ".exit" | ".quit" => break,
                ".help" => {
                    print_repl_help();
                    continue;
                }
                ".clear" => {
                    print!("\x1b[2J\x1b[H");
                    io::stdout().flush().map_err(|e| e.to_string())?;
                    continue;
                }
                _ => {}
            }
        }

        if !buffer.is_empty() {
            buffer.push('\n');
        }
        buffer.push_str(line.trim_end_matches(['\r', '\n']));

        match runtime.run_repl_source(&buffer) {
            Ok(result) => {
                if let Some(value) = result {
                    println!("{}", value.to_text());
                }
                buffer.clear();
            }
            Err(err) if is_incomplete_repl_input(&err) => {}
            Err(err) => {
                eprintln!("{}:\n{}", error_label("Void REPL error"), error_text(&err));
                buffer.clear();
            }
        }
    }

    Ok(())
}

fn is_incomplete_repl_input(err: &str) -> bool {
    err.contains("Unterminated block")
        || err.contains("Expected '}'")
        || err.contains("Expected ')'")
        || err.contains("Expected expression")
}

fn print_repl_help() {
    println!("{}", info_label("Void REPL Commands"));
    println!("{}", info_text("  .help   Show REPL help"));
    println!("{}", info_text("  .exit   Exit REPL"));
    println!("{}", info_text("  .quit   Exit REPL"));
    println!("{}", info_text("  .clear  Clear terminal"));
}
