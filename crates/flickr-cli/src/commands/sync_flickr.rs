use std::path::PathBuf;

use anyhow::Result;
use flickr_api::{FlickrAuth, FlickrClient};
use indicatif::{ProgressBar, ProgressStyle};

use crate::config::Config;

pub async fn run(directory: String, max_images: Option<usize>) -> Result<()> {
    let config = Config::from_env()?;
    let auth = FlickrAuth::new(config.flickr_api_key.clone(), config.flickr_api_secret);

    let tokens = auth.load_tokens()?;
    let client = FlickrClient::new(auth, tokens, config.flickr_api_key);

    let user_id = client.get_user_id().await?;
    let photos = client.fetch_all_photos(&user_id, max_images).await?;

    let dir_path = PathBuf::from(&directory);
    if !dir_path.exists() {
        tokio::fs::create_dir_all(&dir_path).await?;
    }

    println!("Downloading {} photos to {}", photos.len(), directory);

    let pb = ProgressBar::new(photos.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("=> "),
    );
    pb.set_message("Downloading photos");

    let mut downloaded = 0u64;
    let mut skipped = 0u64;
    let mut failed = 0u64;

    for photo in &photos {
        let dest = dir_path.join(format!("{}.jpg", photo.id));
        match client.download_photo(photo, &dest).await {
            Ok(true) => downloaded += 1,
            Ok(false) => skipped += 1, // already exists or no URL
            Err(e) => {
                eprintln!("Error downloading ID {}: {}", photo.id, e);
                failed += 1;
            }
        }
        pb.inc(1);
    }
    pb.finish_and_clear();

    println!(
        "Sync complete: {} downloaded, {} skipped, {} failed",
        downloaded, skipped, failed
    );

    Ok(())
}
