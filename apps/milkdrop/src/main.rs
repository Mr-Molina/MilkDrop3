//! MilkDrop application entry point (`render` subcommand; `run` arrives with
//! the windowed app in Milestone 8).
#![forbid(unsafe_code)]

use milkdrop::cli::{parse_args, Command, USAGE};
use std::process::ExitCode;

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&argv) {
        Ok(Command::Help) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Command::Render(args)) => match milkdrop::render_cmd::run(&args) {
            Ok(paths) => {
                for p in paths {
                    println!("{}", p.display());
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("milkdrop: {e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("milkdrop: {e}\n{USAGE}");
            ExitCode::from(2)
        }
    }
}
