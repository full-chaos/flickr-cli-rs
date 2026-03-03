use anyhow::{bail, Result};

/// Application configuration loaded from environment variables.
pub struct Config {
    pub flickr_api_key: String,
    pub flickr_api_secret: String,
}

impl Config {
    /// Load configuration from environment variables.
    /// Requires FLICKR_API_KEY and FLICKR_API_SECRET.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("FLICKR_API_KEY")
            .map_err(|_| anyhow::anyhow!("FLICKR_API_KEY environment variable not set"))?;
        let api_secret = std::env::var("FLICKR_API_SECRET")
            .map_err(|_| anyhow::anyhow!("FLICKR_API_SECRET environment variable not set"))?;

        if api_key.is_empty() || api_secret.is_empty() {
            bail!("FLICKR_API_KEY and FLICKR_API_SECRET must not be empty");
        }

        Ok(Self {
            flickr_api_key: api_key,
            flickr_api_secret: api_secret,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize all env-var tests so they don't race each other.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_config_from_env_success() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::set_var("FLICKR_API_KEY", "test_key");
        std::env::set_var("FLICKR_API_SECRET", "test_secret");
        let result = Config::from_env();
        std::env::remove_var("FLICKR_API_KEY");
        std::env::remove_var("FLICKR_API_SECRET");
        let config = result.unwrap();
        assert_eq!(config.flickr_api_key, "test_key");
        assert_eq!(config.flickr_api_secret, "test_secret");
    }

    #[test]
    fn test_config_missing_api_key() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::remove_var("FLICKR_API_KEY");
        std::env::set_var("FLICKR_API_SECRET", "test_secret");
        let result = Config::from_env();
        std::env::remove_var("FLICKR_API_SECRET");
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("FLICKR_API_KEY"));
    }

    #[test]
    fn test_config_missing_api_secret() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::set_var("FLICKR_API_KEY", "test_key");
        std::env::remove_var("FLICKR_API_SECRET");
        let result = Config::from_env();
        std::env::remove_var("FLICKR_API_KEY");
        assert!(result.is_err());
        let msg = result.err().unwrap().to_string();
        assert!(msg.contains("FLICKR_API_SECRET"));
    }

    #[test]
    fn test_config_empty_api_key() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::set_var("FLICKR_API_KEY", "");
        std::env::set_var("FLICKR_API_SECRET", "test_secret");
        let result = Config::from_env();
        std::env::remove_var("FLICKR_API_KEY");
        std::env::remove_var("FLICKR_API_SECRET");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_empty_api_secret() {
        let _lock = ENV_MUTEX.lock().unwrap();
        std::env::set_var("FLICKR_API_KEY", "test_key");
        std::env::set_var("FLICKR_API_SECRET", "");
        let result = Config::from_env();
        std::env::remove_var("FLICKR_API_KEY");
        std::env::remove_var("FLICKR_API_SECRET");
        assert!(result.is_err());
    }
}
