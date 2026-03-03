use std::path::Path;

use anyhow::Result;
use dedupe_engine::{DedupeEngine, DedupeMethod};

pub fn run(
    directory: String,
    method: Option<String>,
    model: Option<String>,
    max_images: Option<usize>,
    similarity_threshold: f32,
) -> Result<()> {
    let dir = Path::new(&directory);
    if !dir.is_dir() {
        anyhow::bail!("{} is not a valid directory", directory);
    }

    // Resolve method: explicit choice or auto-select
    let dedupe_method = if let Some(m) = method {
        m.parse::<DedupeMethod>().map_err(|e| anyhow::anyhow!(e))?
    } else {
        let m = DedupeEngine::auto_select().ok_or_else(|| {
            anyhow::anyhow!("No deduplication methods available. Enable 'onnx' or 'phash' feature.")
        })?;
        println!("Auto-selected method: {}", m);
        m
    };

    let pairs = DedupeEngine::find_duplicates(
        dedupe_method,
        dir,
        similarity_threshold,
        max_images,
        model.as_deref(),
    )?;

    if pairs.is_empty() {
        println!(
            "No duplicates found above threshold {:.2}",
            similarity_threshold
        );
    } else {
        println!("\nFound {} duplicate pairs:", pairs.len());
        for pair in &pairs {
            let name_a = pair
                .path_a
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let name_b = pair
                .path_b
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            println!("[{:.2}] {} <-> {}", pair.similarity, name_a, name_b);
        }
    }

    Ok(())
}
