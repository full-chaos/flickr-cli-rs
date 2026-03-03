use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use dedupe_engine::DedupeEngine;

pub fn run(directory: String, num_images: usize, model: Option<String>) -> Result<()> {
    let dir = Path::new(&directory);
    if !dir.is_dir() {
        anyhow::bail!("{} is not a valid directory", directory);
    }

    let available = DedupeEngine::available_methods();
    if available.is_empty() {
        anyhow::bail!("No deduplication methods available.");
    }

    println!(
        "Benchmarking with up to {} images from {}",
        num_images, directory
    );
    println!("\n{}", "=".repeat(50));
    println!("BENCHMARK RESULTS");
    println!("{}", "=".repeat(50));

    for method in &available {
        println!("\nTesting {}...", method);

        let start = Instant::now();
        match DedupeEngine::find_duplicates(*method, dir, 0.95, Some(num_images), model.as_deref())
        {
            Ok(pairs) => {
                let elapsed = start.elapsed();
                let per_image = elapsed.as_secs_f64() / num_images as f64;
                println!("  Duplicates found: {}", pairs.len());
                println!("  Total time: {:.2}s", elapsed.as_secs_f64());
                println!("  Per image: {:.3}s", per_image);
                if per_image > 0.0 {
                    println!("  Images/sec: {:.1}", 1.0 / per_image);
                }
            }
            Err(e) => {
                println!("  Error: {}", e);
            }
        }
    }

    println!("\n{}", "=".repeat(50));

    Ok(())
}
