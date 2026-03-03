use std::collections::HashMap;

use anyhow::Result;
use flickr_api::{FlickrAuth, FlickrClient, Photo};

use crate::cli::ScanBy;
use crate::config::Config;

pub async fn run(by: Vec<ScanBy>) -> Result<()> {
    let config = Config::from_env()?;
    let auth = FlickrAuth::new(config.flickr_api_key.clone(), config.flickr_api_secret);

    let tokens = auth.load_tokens()?;
    let client = FlickrClient::new(auth, tokens, config.flickr_api_key);

    let user_id = client.get_user_id().await?;
    let photos = client.fetch_all_photos(&user_id, None).await?;
    println!("Total photos fetched: {}", photos.len());

    if by.contains(&ScanBy::Title) {
        print_duplicates_by("title", &photos, |p| p.title.clone());
    }

    if by.contains(&ScanBy::Filename) {
        print_duplicates_by("filename", &photos, |p| {
            p.originalformat.clone().unwrap_or_default()
        });
    }

    if by.contains(&ScanBy::Datetaken) {
        print_duplicates_by("date taken", &photos, |p| {
            p.datetaken
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(10) // YYYY-MM-DD
                .collect()
        });
    }

    Ok(())
}

fn print_duplicates_by(label: &str, photos: &[Photo], key_fn: impl Fn(&Photo) -> String) {
    let mut groups: HashMap<String, Vec<&Photo>> = HashMap::new();
    for photo in photos {
        let key = key_fn(photo);
        if !key.is_empty() {
            groups.entry(key).or_default().push(photo);
        }
    }

    println!("\nDuplicates by {}:", label);
    let mut found = false;
    for (key, group) in &groups {
        if group.len() > 1 {
            found = true;
            println!("\n  {} ({} photos):", key, group.len());
            for p in group {
                let date = p.datetaken.as_deref().unwrap_or("?");
                println!("    - ID: {} | Title: {} | Date: {}", p.id, p.title, date);
            }
        }
    }
    if !found {
        println!("  No duplicates found.");
    }
}
