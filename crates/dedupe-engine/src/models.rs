//! Known CLIP/SigLIP model configurations for image deduplication.
//!
//! Each model has its own input size, normalization constants, and embedding
//! dimension. The Immich project maintains ONNX exports of these on HuggingFace.

/// Preprocessing and inference configuration for a vision model.
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Human-readable name (e.g. "siglip2-b16")
    pub name: &'static str,
    /// HuggingFace repo ID for the ONNX visual encoder
    pub hf_repo: &'static str,
    /// Filename of the ONNX model within the repo
    pub hf_filename: &'static str,
    /// Input image size (square: size x size)
    pub input_size: u32,
    /// Per-channel mean for normalization [R, G, B]
    pub mean: [f32; 3],
    /// Per-channel std for normalization [R, G, B]
    pub std: [f32; 3],
    /// Output embedding dimensionality
    pub embedding_dim: usize,
}

/// OpenAI CLIP ViT-B/32 — the original default. Fast, 512-dim.
pub const CLIP_VIT_B32: ModelConfig = ModelConfig {
    name: "clip-vit-b32",
    hf_repo: "immich-app/ViT-B-32__openai",
    hf_filename: "visual/model.onnx",
    input_size: 224,
    mean: [0.48145466, 0.4578275, 0.40821073],
    std: [0.26862954, 0.26130258, 0.27577711],
    embedding_dim: 512,
};

/// SigLIP2 ViT-B/16 — best quality/cost ratio. 768-dim, 224px input.
/// ~86% recall (vs ~75% for ViT-B-32). Feb 2025.
pub const SIGLIP2_VIT_B16: ModelConfig = ModelConfig {
    name: "siglip2-b16",
    hf_repo: "immich-app/ViT-B-16-SigLIP2__webli",
    hf_filename: "visual/model.onnx",
    input_size: 224,
    // SigLIP2 uses 0.5/0.5 normalization (standard ImageNet-like)
    mean: [0.5, 0.5, 0.5],
    std: [0.5, 0.5, 0.5],
    embedding_dim: 768,
};

/// SigLIP2 SO400M — highest quality. 1152-dim, 384px input, ~7GB RAM.
/// ~86% recall, best semantic understanding. Feb 2025.
pub const SIGLIP2_SO400M: ModelConfig = ModelConfig {
    name: "siglip2-so400m",
    hf_repo: "immich-app/ViT-SO400M-16-SigLIP2-384__webli",
    hf_filename: "visual/model.onnx",
    input_size: 384,
    mean: [0.5, 0.5, 0.5],
    std: [0.5, 0.5, 0.5],
    embedding_dim: 1152,
};

/// All known models, ordered by quality (best last for auto-selection).
pub const ALL_MODELS: &[&ModelConfig] = &[&CLIP_VIT_B32, &SIGLIP2_VIT_B16, &SIGLIP2_SO400M];

/// Default model — SigLIP2 B/16 is the recommended sweet spot.
pub const DEFAULT_MODEL: &ModelConfig = &SIGLIP2_VIT_B16;

/// Look up a model config by name.
pub fn find_model(name: &str) -> Option<&'static ModelConfig> {
    let name_lower = name.to_lowercase();
    match name_lower.as_str() {
        // Exact matches
        "clip-vit-b32" | "clip" | "vit-b-32" | "openai" => Some(&CLIP_VIT_B32),
        "siglip2-b16" | "siglip2" | "siglip" | "vit-b-16-siglip2" => Some(&SIGLIP2_VIT_B16),
        "siglip2-so400m" | "so400m" | "vit-so400m" | "large" => Some(&SIGLIP2_SO400M),
        // Legacy: "immich" maps to the current best default (was ViT-B-32, now SigLIP2)
        "immich" => Some(DEFAULT_MODEL),
        _ => None,
    }
}

/// List all available model names for CLI help.
pub fn model_names() -> Vec<&'static str> {
    ALL_MODELS.iter().map(|m| m.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_model_exact_names() {
        assert!(find_model("clip-vit-b32").is_some());
        assert_eq!(find_model("clip-vit-b32").unwrap().name, "clip-vit-b32");
        assert!(find_model("siglip2-b16").is_some());
        assert_eq!(find_model("siglip2-b16").unwrap().name, "siglip2-b16");
        assert!(find_model("siglip2-so400m").is_some());
        assert_eq!(find_model("siglip2-so400m").unwrap().name, "siglip2-so400m");
    }

    #[test]
    fn find_model_aliases() {
        assert_eq!(find_model("clip").unwrap().name, "clip-vit-b32");
        assert_eq!(find_model("openai").unwrap().name, "clip-vit-b32");
        assert_eq!(find_model("siglip").unwrap().name, "siglip2-b16");
        assert_eq!(find_model("siglip2").unwrap().name, "siglip2-b16");
        assert_eq!(find_model("so400m").unwrap().name, "siglip2-so400m");
        assert_eq!(find_model("large").unwrap().name, "siglip2-so400m");
        // "immich" maps to DEFAULT_MODEL which is siglip2-b16
        assert_eq!(find_model("immich").unwrap().name, DEFAULT_MODEL.name);
    }

    #[test]
    fn find_model_case_insensitive() {
        assert_eq!(find_model("CLIP-VIT-B32").unwrap().name, "clip-vit-b32");
        assert_eq!(find_model("SigLIP2-B16").unwrap().name, "siglip2-b16");
        assert_eq!(find_model("CLIP").unwrap().name, "clip-vit-b32");
        assert_eq!(find_model("SIGLIP2").unwrap().name, "siglip2-b16");
    }

    #[test]
    fn find_model_unknown_returns_none() {
        assert!(find_model("unknown-model").is_none());
        assert!(find_model("").is_none());
        assert!(find_model("vgg16").is_none());
        assert!(find_model("resnet50").is_none());
    }

    #[test]
    fn model_names_returns_three() {
        let names = model_names();
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn default_model_is_siglip2_b16() {
        assert_eq!(DEFAULT_MODEL.name, "siglip2-b16");
    }

    #[test]
    fn clip_vit_b32_config_values() {
        assert_eq!(CLIP_VIT_B32.input_size, 224);
        assert_eq!(CLIP_VIT_B32.embedding_dim, 512);
        assert_eq!(CLIP_VIT_B32.name, "clip-vit-b32");
    }

    #[test]
    fn siglip2_vit_b16_config_values() {
        assert_eq!(SIGLIP2_VIT_B16.input_size, 224);
        assert_eq!(SIGLIP2_VIT_B16.embedding_dim, 768);
        assert_eq!(SIGLIP2_VIT_B16.name, "siglip2-b16");
    }

    #[test]
    fn siglip2_so400m_config_values() {
        assert_eq!(SIGLIP2_SO400M.input_size, 384);
        assert_eq!(SIGLIP2_SO400M.embedding_dim, 1152);
        assert_eq!(SIGLIP2_SO400M.name, "siglip2-so400m");
    }

    #[test]
    fn all_models_has_exactly_three_entries() {
        assert_eq!(ALL_MODELS.len(), 3);
    }
}
