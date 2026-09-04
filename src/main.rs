//! CLI wrapper. All the actual logic lives in the library and stays pure;
//! this file is the only place that touches the filesystem or exits the
//! process.

use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let paths: Vec<String> = env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: sqlmig-lint <file.sql> [more.sql ...]");
        return ExitCode::FAILURE;
    }

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

        for finding in sqlmig_lint::lint(&source) {
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
