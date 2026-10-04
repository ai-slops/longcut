#[path = "../tests/common/mod.rs"]
mod common;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os()
        .nth(1)
        .ok_or("usage: make_fixture NEW_DIRECTORY")?;
    let root = std::path::PathBuf::from(root);
    std::fs::create_dir(&root)?;
    common::prepare(&root).await?;
    println!("Synthetic 10-second package created at {}", root.display());
    Ok(())
}
