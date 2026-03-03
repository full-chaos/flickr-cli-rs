#[cfg(feature = "download")]
pub mod download;
pub mod engine;
pub mod models;
#[cfg(feature = "onnx")]
pub mod onnx;
#[cfg(feature = "phash")]
pub mod phash;
#[cfg(feature = "onnx")]
pub mod preprocess;
pub mod similarity;

pub use engine::{DedupeEngine, DedupeMethod, DuplicatePair};
pub use models::{find_model, model_names, ModelConfig};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DedupeError {
    #[error("Image loading error: {0}")]
    Image(#[from] image::ImageError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("No images found in directory")]
    NoImages,

    #[error("Method not available: {0}")]
    MethodNotAvailable(String),

    #[cfg(feature = "onnx")]
    #[error("ONNX runtime error: {0}")]
    Onnx(#[from] ort::Error),

    #[error("{0}")]
    Other(String),
}
