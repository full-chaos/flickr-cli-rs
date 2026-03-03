use anyhow::Result;
use flickr_api::{FlickrAuth, FlickrClient};
use indicatif::{ProgressBar, ProgressStyle};

use crate::config::Config;

pub async fn run(threshold: u32) -> Result<()> {
    let config = Config::from_env()?;
    let auth = FlickrAuth::new(config.flickr_api_key.clone(), config.flickr_api_secret);

    let tokens = auth.load_tokens()?;
    let client = FlickrClient::new(auth, tokens, config.flickr_api_key);

    let user_id = client.get_user_id().await?;
    let photos = client.fetch_all_photos(&user_id, None).await?;
    println!("Fuzzy matching {} titles...", photos.len());

    let total_combinations = (photos.len() * (photos.len() - 1)) / 2;
    let pb = ProgressBar::new(total_combinations as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("=> "),
    );
    pb.set_message("Comparing photo titles");

    let mut pairs = Vec::new();

    for i in 0..photos.len() {
        for j in (i + 1)..photos.len() {
            let a_title = &photos[i].title;
            let b_title = &photos[j].title;

            // fuzz::ratio matches Python's rapidfuzz.fuzz.ratio (normalized indel similarity)
            let score_f64 = rapidfuzz::fuzz::ratio(a_title.chars(), b_title.chars());
            let score = (score_f64 * 100.0).round() as u32;

            if score >= threshold {
                pairs.push((i, j, score));
            }
            pb.inc(1);
        }
    }
    pb.finish_and_clear();

    if pairs.is_empty() {
        println!("No fuzzy duplicate titles found.");
    } else {
        println!("Found {} fuzzy duplicate pairs:", pairs.len());
        for (i, j, score) in &pairs {
            println!(
                "({}%) '{}' [ID:{}] <--> '{}' [ID:{}]",
                score, photos[*i].title, photos[*i].id, photos[*j].title, photos[*j].id,
            );
        }
    }

    Ok(())
}
