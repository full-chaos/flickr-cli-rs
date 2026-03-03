use std::path::{Path, PathBuf};

use ort::session::Session;

use crate::engine::{progress_bar, DuplicatePair};
use crate::models::ModelConfig;
use crate::preprocess::preprocess_image;
use crate::similarity::{find_similar_pairs, l2_normalize};
use crate::DedupeError;

/// Find duplicates using vision model embeddings via ONNX Runtime.
pub fn find_duplicates_onnx(
    paths: &[PathBuf],
    similarity_threshold: f32,
    model_config: &ModelConfig,
) -> Result<Vec<DuplicatePair>, DedupeError> {
    let mut session = load_onnx_session(model_config)?;

    // Compute embeddings
    let pb = progress_bar(
        paths.len() as u64,
        &format!("Computing {} embeddings", model_config.name),
    );
    let mut embeddings = Vec::with_capacity(paths.len());

    for path in paths {
        let embedding = compute_embedding(&mut session, path, model_config)?;
        embeddings.push(embedding);
        pb.inc(1);
    }
    pb.finish_with_message("Embeddings complete");

    // Find similar pairs
    let pairs = find_similar_pairs(&embeddings, paths, similarity_threshold);
    Ok(pairs)
}

/// Load the ONNX session for the given model, searching local paths then downloading.
fn load_onnx_session(config: &ModelConfig) -> Result<Session, DedupeError> {
    // Build model-specific search paths
    let search_paths = vec![
        format!(
            "cache/clip/{}/visual/model.onnx",
            config.hf_repo.split('/').last().unwrap_or("model")
        ),
        format!("models/{}.onnx", config.name),
        "cache/model.onnx".to_string(),
        "models/model.onnx".to_string(),
    ];

    for path_str in &search_paths {
        let path = Path::new(path_str);
        if path.exists() {
            println!("Loading {} model from {}", config.name, path.display());
            let session = Session::builder()?
                .with_intra_threads(4)?
                .commit_from_file(path)?;
            return Ok(session);
        }
    }

    // Try downloading if the download feature is enabled
    #[cfg(feature = "download")]
    {
        let model_path = crate::download::download_model(config)?;
        println!(
            "Loading {} model from {}",
            config.name,
            model_path.display()
        );
        let session = Session::builder()?
            .with_intra_threads(4)?
            .commit_from_file(&model_path)?;
        return Ok(session);
    }

    #[cfg(not(feature = "download"))]
    Err(DedupeError::Other(format!(
        "{} model not found. Enable the 'download' feature to auto-download from HuggingFace.",
        config.name,
    )))
}

/// Compute an embedding for a single image using the given model config.
fn compute_embedding(
    session: &mut Session,
    path: &Path,
    config: &ModelConfig,
) -> Result<Vec<f32>, DedupeError> {
    let img = image::open(path)?;
    let tensor = preprocess_image(&img, config);
    let raw: Vec<f32> = tensor.as_slice().expect("tensor is contiguous").to_vec();

    let s = config.input_size as usize;
    let input = ort::value::Value::from_array(([1usize, 3, s, s], raw))?;
    let outputs = session.run(ort::inputs![input])?;

    let output = &outputs[0];
    let tensor_ref = output.try_extract_tensor::<f32>()?;
    let mut embedding: Vec<f32> = tensor_ref.1.to_vec();

    l2_normalize(&mut embedding);
    Ok(embedding)
}
