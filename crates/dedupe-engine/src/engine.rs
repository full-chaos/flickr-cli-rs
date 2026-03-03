use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};

use crate::models;
use crate::DedupeError;

/// A pair of duplicate images with their similarity score.
#[derive(Debug, Clone)]
pub struct DuplicatePair {
    pub path_a: PathBuf,
    pub path_b: PathBuf,
    pub similarity: f32,
}

/// Available deduplication methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DedupeMethod {
    /// Perceptual hashing (dHash) — no ML model needed.
    PHash,
    /// Vision model via ONNX Runtime.
    Onnx,
    /// ONNX with CoreML Execution Provider (macOS).
    CoreML,
}

impl DedupeMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PHash => "phash",
            Self::Onnx => "onnx",
            Self::CoreML => "coreml",
        }
    }
}

impl std::fmt::Display for DedupeMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for DedupeMethod {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "phash" | "hash" => Ok(Self::PHash),
            // "immich" maps to ONNX (same backend, better model now)
            "onnx" | "immich" => Ok(Self::Onnx),
            "coreml" => Ok(Self::CoreML),
            other => Err(format!("Unknown method: {}", other)),
        }
    }
}

/// Main deduplication engine. Dispatches to the selected backend.
pub struct DedupeEngine;

impl DedupeEngine {
    /// Return which methods are available at runtime.
    #[allow(clippy::vec_init_then_push)] // pushes are behind #[cfg] flags
    pub fn available_methods() -> Vec<DedupeMethod> {
        let mut methods = Vec::new();

        #[cfg(feature = "phash")]
        methods.push(DedupeMethod::PHash);

        #[cfg(feature = "onnx")]
        methods.push(DedupeMethod::Onnx);

        #[cfg(all(feature = "coreml", target_os = "macos"))]
        methods.push(DedupeMethod::CoreML);

        methods
    }

    /// Auto-select the best available method.
    pub fn auto_select() -> Option<DedupeMethod> {
        let methods = Self::available_methods();
        if methods.contains(&DedupeMethod::CoreML) {
            Some(DedupeMethod::CoreML)
        } else if methods.contains(&DedupeMethod::Onnx) {
            Some(DedupeMethod::Onnx)
        } else if methods.contains(&DedupeMethod::PHash) {
            Some(DedupeMethod::PHash)
        } else {
            None
        }
    }

    /// Find duplicate images in a directory.
    ///
    /// `model_name` selects which vision model to use for ONNX-based methods.
    /// Pass `None` to use the default (SigLIP2 ViT-B/16).
    pub fn find_duplicates(
        method: DedupeMethod,
        dir: &Path,
        similarity_threshold: f32,
        max_images: Option<usize>,
        model_name: Option<&str>,
    ) -> Result<Vec<DuplicatePair>, DedupeError> {
        let image_paths = collect_images(dir, max_images)?;
        if image_paths.is_empty() {
            return Err(DedupeError::NoImages);
        }

        // Resolve model config for ONNX-based methods
        let model_config = match model_name {
            Some(name) => models::find_model(name).ok_or_else(|| {
                DedupeError::Other(format!(
                    "Unknown model '{}'. Available: {}",
                    name,
                    models::model_names().join(", ")
                ))
            })?,
            None => models::DEFAULT_MODEL,
        };

        println!(
            "Processing {} images with {} method{}...",
            image_paths.len(),
            method,
            if method == DedupeMethod::PHash {
                String::new()
            } else {
                format!(" (model: {})", model_config.name)
            }
        );

        match method {
            #[cfg(feature = "phash")]
            DedupeMethod::PHash => {
                crate::phash::find_duplicates_phash(&image_paths, similarity_threshold)
            }
            #[cfg(feature = "onnx")]
            DedupeMethod::Onnx => {
                crate::onnx::find_duplicates_onnx(&image_paths, similarity_threshold, model_config)
            }
            #[cfg(all(feature = "coreml", target_os = "macos"))]
            DedupeMethod::CoreML => {
                crate::onnx::find_duplicates_onnx(&image_paths, similarity_threshold, model_config)
            }
            #[allow(unreachable_patterns)]
            _ => Err(DedupeError::MethodNotAvailable(method.to_string())),
        }
    }
}

/// Collect JPEG/PNG images from a directory (non-recursive).
pub fn collect_images(dir: &Path, max_images: Option<usize>) -> Result<Vec<PathBuf>, DedupeError> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension()?.to_str()?.to_lowercase();
                if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") {
                    return Some(path);
                }
            }
            None
        })
        .collect();

    paths.sort();

    if let Some(max) = max_images {
        paths.truncate(max);
    }

    Ok(paths)
}

/// Create a progress bar with a consistent style.
pub fn progress_bar(len: u64, msg: &str) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("=> "),
    );
    pb.set_message(msg.to_string());
    pb
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use tempfile::TempDir;

    // --- DedupeMethod::from_str ---

    #[test]
    fn from_str_phash() {
        assert_eq!(
            DedupeMethod::from_str("phash").unwrap(),
            DedupeMethod::PHash
        );
    }

    #[test]
    fn from_str_hash_alias() {
        assert_eq!(DedupeMethod::from_str("hash").unwrap(), DedupeMethod::PHash);
    }

    #[test]
    fn from_str_onnx() {
        assert_eq!(DedupeMethod::from_str("onnx").unwrap(), DedupeMethod::Onnx);
    }

    #[test]
    fn from_str_coreml() {
        assert_eq!(
            DedupeMethod::from_str("coreml").unwrap(),
            DedupeMethod::CoreML
        );
    }

    #[test]
    fn from_str_immich_alias() {
        assert_eq!(
            DedupeMethod::from_str("immich").unwrap(),
            DedupeMethod::Onnx
        );
    }

    #[test]
    fn from_str_uppercase() {
        assert_eq!(
            DedupeMethod::from_str("PHASH").unwrap(),
            DedupeMethod::PHash
        );
        assert_eq!(DedupeMethod::from_str("ONNX").unwrap(), DedupeMethod::Onnx);
    }

    #[test]
    fn from_str_unknown_returns_err() {
        assert!(DedupeMethod::from_str("unknown").is_err());
        assert!(DedupeMethod::from_str("").is_err());
        assert!(DedupeMethod::from_str("neural").is_err());
    }

    // --- DedupeMethod::as_str ---

    #[test]
    fn as_str_phash() {
        assert_eq!(DedupeMethod::PHash.as_str(), "phash");
    }

    #[test]
    fn as_str_onnx() {
        assert_eq!(DedupeMethod::Onnx.as_str(), "onnx");
    }

    #[test]
    fn as_str_coreml() {
        assert_eq!(DedupeMethod::CoreML.as_str(), "coreml");
    }

    // --- DedupeMethod Display ---

    #[test]
    fn display_matches_as_str() {
        assert_eq!(format!("{}", DedupeMethod::PHash), "phash");
        assert_eq!(format!("{}", DedupeMethod::Onnx), "onnx");
        assert_eq!(format!("{}", DedupeMethod::CoreML), "coreml");
    }

    // --- DedupeEngine::available_methods ---

    #[test]
    fn available_methods_non_empty() {
        let methods = DedupeEngine::available_methods();
        assert!(
            !methods.is_empty(),
            "at least one method should be available"
        );
    }

    #[test]
    fn available_methods_contains_phash() {
        // phash is the default feature, so it should always be available
        let methods = DedupeEngine::available_methods();
        assert!(
            methods.contains(&DedupeMethod::PHash),
            "PHash should be available with default features"
        );
    }

    // --- DedupeEngine::auto_select ---

    #[test]
    fn auto_select_returns_some() {
        // At least PHash is compiled in via default features
        let method = DedupeEngine::auto_select();
        assert!(method.is_some(), "auto_select should return at least PHash");
    }

    // --- collect_images ---

    fn make_temp_image(dir: &TempDir, name: &str) {
        use image::{ImageBuffer, RgbImage};
        let img: RgbImage = ImageBuffer::new(1, 1);
        img.save(dir.path().join(name)).unwrap();
    }

    #[test]
    fn collect_images_finds_jpg_png_webp() {
        let dir = TempDir::new().unwrap();
        make_temp_image(&dir, "test1.jpg");
        make_temp_image(&dir, "test2.png");
        make_temp_image(&dir, "test3.webp");
        // Non-image files that should be excluded
        std::fs::write(dir.path().join("test4.txt"), b"hello").unwrap();
        std::fs::write(dir.path().join("test5.gif"), b"GIF89a").unwrap();

        let paths = collect_images(dir.path(), None).unwrap();
        assert_eq!(paths.len(), 3, "expected 3 image files, got {:?}", paths);
    }

    #[test]
    fn collect_images_excludes_non_images() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("readme.txt"), b"text").unwrap();
        std::fs::write(dir.path().join("data.gif"), b"GIF89a").unwrap();
        std::fs::write(dir.path().join("archive.zip"), b"PK").unwrap();

        let paths = collect_images(dir.path(), None).unwrap();
        assert!(
            paths.is_empty(),
            "no image files should be found, got {:?}",
            paths
        );
    }

    #[test]
    fn collect_images_sorted() {
        let dir = TempDir::new().unwrap();
        make_temp_image(&dir, "c.jpg");
        make_temp_image(&dir, "a.jpg");
        make_temp_image(&dir, "b.jpg");

        let paths = collect_images(dir.path(), None).unwrap();
        assert_eq!(paths.len(), 3);
        let names: Vec<_> = paths
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, vec!["a.jpg", "b.jpg", "c.jpg"]);
    }

    #[test]
    fn collect_images_max_images_limits_results() {
        let dir = TempDir::new().unwrap();
        make_temp_image(&dir, "a.jpg");
        make_temp_image(&dir, "b.jpg");
        make_temp_image(&dir, "c.jpg");
        make_temp_image(&dir, "d.png");

        let paths = collect_images(dir.path(), Some(2)).unwrap();
        assert_eq!(paths.len(), 2, "max_images=2 should limit to 2 results");
    }

    #[test]
    fn collect_images_empty_directory_returns_empty() {
        let dir = TempDir::new().unwrap();
        let paths = collect_images(dir.path(), None).unwrap();
        assert!(paths.is_empty());
    }

    #[test]
    fn collect_images_jpeg_extension_included() {
        let dir = TempDir::new().unwrap();
        make_temp_image(&dir, "photo.jpeg");
        let paths = collect_images(dir.path(), None).unwrap();
        assert_eq!(paths.len(), 1);
    }

    // --- progress_bar ---

    #[test]
    fn progress_bar_does_not_panic() {
        let pb = progress_bar(100, "test");
        // Just verify it was created; finish it to avoid output noise.
        pb.finish_and_clear();
    }
}
