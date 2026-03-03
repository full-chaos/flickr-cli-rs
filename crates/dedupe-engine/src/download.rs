use std::path::PathBuf;

use crate::models::ModelConfig;
use crate::DedupeError;

/// Download a vision model's ONNX file from HuggingFace Hub.
///
/// Uses the hf-hub crate which is cache-compatible with the Python
/// huggingface_hub library, so models downloaded by either tool are shared.
pub fn download_model(config: &ModelConfig) -> Result<PathBuf, DedupeError> {
    use hf_hub::api::sync::Api;

    println!(
        "Downloading {} model from {}...",
        config.name, config.hf_repo
    );

    let api =
        Api::new().map_err(|e| DedupeError::Other(format!("HuggingFace API error: {}", e)))?;
    let repo = api.model(config.hf_repo.to_string());

    let model_path = repo
        .get(config.hf_filename)
        .map_err(|e| DedupeError::Other(format!("Failed to download {}: {}", config.name, e)))?;

    println!("Model downloaded to {}", model_path.display());
    Ok(model_path)
}
