use std::{
    io::{self, Write},
    path::Path,
    process::ExitCode,
};

use longcut_session_contract::{Conformance, inspect, schema::schemas};

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let mut stdout = io::stdout().lock();
    match args.as_slice() {
        [command] if command == "--help" || command == "-h" => {
            writeln!(
                stdout,
                "session-inspect inspect PACKAGE_DIRECTORY\nsession-inspect schema\nMetadata-only validation. Media decode/checksum/sync are NOT verified.\nExit: 0 conformant metadata/schema/help; 1 degraded; 2 rejected/usage."
            )?;
            Ok(ExitCode::SUCCESS)
        }
        [command] if command == "schema" => {
            serde_json::to_writer_pretty(&mut stdout, &schemas())?;
            writeln!(stdout)?;
            Ok(ExitCode::SUCCESS)
        }
        [command, directory] if command == "inspect" => {
            let report = inspect(Path::new(directory));
            serde_json::to_writer_pretty(&mut stdout, &report)?;
            writeln!(stdout)?;
            Ok(ExitCode::from(match report.metadata_conformance {
                Conformance::Conformant => 0,
                Conformance::Degraded => 1,
                Conformance::Rejected => 2,
            }))
        }
        _ => {
            eprintln!("usage: session-inspect inspect PACKAGE_DIRECTORY | schema | --help");
            Ok(ExitCode::from(2))
        }
    }
}
