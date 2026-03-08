mod ast;
mod lexer;
mod parser;
mod runtime;
mod value;

use std::path::Path;

use runtime::Runtime;

const VERSION: &str = "0.1.2";

fn print_help() {
    println!(
        "Void {VERSION}\nUsage:\n  void <file-or-dir> [args...]\n\nOptions:\n  --help, -h       Show help\n  --version, -v    Show version"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let first = args.first().map(String::as_str);
    match first {
        None | Some("--help") | Some("-h") => {
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
                eprintln!("Void runtime error:\n{err}");
                std::process::exit(1);
            }
        }
    }
}
