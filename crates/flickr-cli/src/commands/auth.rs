use anyhow::Result;
use flickr_api::FlickrAuth;

use crate::config::Config;

pub async fn run() -> Result<()> {
    let config = Config::from_env()?;
    let auth = FlickrAuth::new(config.flickr_api_key, config.flickr_api_secret);

    let tokens = auth.oauth_flow().await?;

    println!("Your OAuth tokens:");
    println!("  oauth_token: {}", tokens.oauth_token);
    println!("  oauth_token_secret: {}", tokens.oauth_token_secret);
    if let Some(ref name) = tokens.fullname {
        println!("  fullname: {}", name);
    }
    if let Some(ref username) = tokens.username {
        println!("  username: {}", username);
    }

    auth.save_tokens(&tokens)?;
    println!("Authentication successful!");

    Ok(())
}
