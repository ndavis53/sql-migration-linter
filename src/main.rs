//! CLI wrapper. All the actual logic lives in the library and stays pure;
//! this file is the only place that touches the filesystem or exits the
//! process.

use std::env;
use std::fs;
use std::process::ExitCode;

use sqlmig_lint::config::Config;

fn main() -> ExitCode {
    let mut config_path: Option<String> = None;
    let mut paths: Vec<String> = Vec::new();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--config" {
            let Some(path) = args.next() else {
                eprintln!("--config requires a path");
                return ExitCode::FAILURE;
            };
            config_path = Some(path);
        } else {
            paths.push(arg);
        }
    }

    if paths.is_empty() {
        eprintln!("usage: sqlmig-lint [--config <file>] <file.sql> [more.sql ...]");
        return ExitCode::FAILURE;
    }

    let config = match config_path {
        Some(path) => {
            let text = match fs::read_to_string(&path) {
                Ok(text) => text,
                Err(err) => {
                    eprintln!("{path}: {err}");
                    return ExitCode::FAILURE;
                }
            };
            match Config::parse(&text) {
                Ok(config) => config,
                Err(err) => {
                    eprintln!("{path}: {err}");
                    return ExitCode::FAILURE;
                }
            }
        }
        None => Config::default(),
    };

    let mut found_anything = false;
    for path in &paths {
        let source = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(err) => {
                eprintln!("{path}: {err}");
                found_anything = true;
                continue;
            }
        };

        for finding in sqlmig_lint::lint_with_config(&source, &config) {
            found_anything = true;
            println!("{path}:{}: [{}] {}", finding.line, finding.rule, finding.message);
        }
    }

    if found_anything {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
