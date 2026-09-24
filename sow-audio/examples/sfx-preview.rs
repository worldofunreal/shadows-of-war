use std::error::Error;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let output_dir = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/sfx-preview"));
    let count = sow_audio::export_sfx_preview(&output_dir)?;
    println!(
        "Exported {count} runtime SFX previews to {}",
        output_dir.display()
    );
    Ok(())
}
