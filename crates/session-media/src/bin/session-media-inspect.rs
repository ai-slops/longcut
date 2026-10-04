use std::{
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use longcut_session_contract::Conformance;
use longcut_session_media::{DecodePolicy, Options, verify};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() || args.len() > 2 {
        eprintln!("usage: session-media-inspect [--full-decode] PACKAGE_DIRECTORY | --help");
        return ExitCode::from(2);
    }
    if args[0] == "--help" || args[0] == "-h" {
        println!(
            "session-media-inspect [--full-decode] PACKAGE_DIRECTORY\nRead-only SHA-256/size/ffprobe, packet continuity, and TimeMap bounds preflight.\n--full-decode additionally decodes every mapped stream with bounded FFmpeg; it does not prove AV alignment.\nOnly sha256:<hex> checksums and standalone file containers are supported.\nExit: 0 preflight conformant; 1 degraded; 2 rejected/usage; 130 interrupted."
        );
        return ExitCode::SUCCESS;
    }
    let (root, decode) = match args.as_slice() {
        [root] => (PathBuf::from(root), DecodePolicy::Skip),
        [flag, root] if flag == "--full-decode" => (PathBuf::from(root), DecodePolicy::Full),
        _ => {
            eprintln!("usage: session-media-inspect [--full-decode] PACKAGE_DIRECTORY | --help");
            return ExitCode::from(2);
        }
    };
    let options = Options {
        decode,
        ..Options::default()
    };
    let inspection = verify(&root, &options);
    tokio::pin!(inspection);
    let (report, interruption) = tokio::select! {
        report = &mut inspection => (report, None),
        signal = tokio::signal::ctrl_c() => {
            let code = match signal { Ok(()) => 130, Err(error) => { eprintln!("signal handler failed: {error}"); 2 } };
            options.cancellation.cancel();
            (inspection.await, Some(code))
        }
    };
    let mut stdout = io::stdout().lock();
    if let Err(error) = serde_json::to_writer_pretty(&mut stdout, &report) {
        eprintln!("cannot write report: {error}");
        return ExitCode::from(2);
    }
    if let Err(error) = writeln!(stdout) {
        eprintln!("cannot write report: {error}");
        return ExitCode::from(2);
    }
    ExitCode::from(interruption.unwrap_or(match report.preflight_conformance {
        Conformance::Conformant => 0,
        Conformance::Degraded => 1,
        Conformance::Rejected => 2,
    }))
}
