#[path = "../tests/common/mod.rs"]
mod common;

use longcut_session_contract::model::{Integer, TimeMaps};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os()
        .nth(1)
        .ok_or("usage: make_out_of_bounds_fixture NEW_DIRECTORY")?;
    let root = std::path::PathBuf::from(root);
    std::fs::create_dir(&root)?;
    common::prepare(&root).await?;
    let path = root.join("time-maps.json");
    let mut maps: TimeMaps = serde_json::from_slice(&std::fs::read(&path)?)?;
    maps.segments[0].media_range.end.ticks = Integer::new(20_000_000_001);
    maps.segments[0].media_range.end.timebase_den = 2_000_000_000;
    std::fs::write(path, serde_json::to_vec_pretty(&maps)?)?;
    println!(
        "Synthetic package with a 0.5 ns source-range overrun: {}",
        root.display()
    );
    Ok(())
}
