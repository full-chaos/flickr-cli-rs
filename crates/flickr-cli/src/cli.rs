use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "flickr-cli",
    about = "Flickr CLI Tool (dedupe, sync, scan, etc)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Authenticate with Flickr via OAuth 1.0a
    Auth,

    /// Scan Flickr for duplicate photos by metadata fields
    Scan {
        /// Fields to group by for duplicate detection
        #[arg(long, value_delimiter = ',', default_values_t = vec![ScanBy::Title, ScanBy::Filename])]
        by: Vec<ScanBy>,
    },

    /// Fuzzy duplicate scan for photo titles using RapidFuzz
    FuzzyScan {
        /// Fuzzy match threshold (0-100)
        #[arg(long, default_value_t = 85)]
        threshold: u32,
    },

    /// Download all Flickr photos to a local directory
    SyncFlickr {
        /// Directory to save downloaded images
        #[arg(long)]
        directory: String,

        /// Maximum number of images to sync (default: all)
        #[arg(long)]
        max_images: Option<usize>,
    },

    /// AI-based duplicate detection on local images
    AiDedupe {
        /// Local directory containing images
        #[arg(long)]
        directory: String,

        /// Deduplication method: phash, onnx, coreml
        #[arg(long)]
        method: Option<String>,

        /// Vision model for ONNX methods: clip-vit-b32, siglip2-b16 (default), siglip2-so400m
        #[arg(long)]
        model: Option<String>,

        /// Maximum number of images to process
        #[arg(long)]
        max_images: Option<usize>,

        /// Similarity threshold (0.0 to 1.0)
        #[arg(long, default_value_t = 0.95)]
        similarity_threshold: f32,
    },

    /// Benchmark available deduplication methods
    BenchmarkMethods {
        /// Directory containing test images
        #[arg(long)]
        directory: String,

        /// Number of images to benchmark
        #[arg(long, default_value_t = 10)]
        num_images: usize,

        /// Vision model to benchmark (default: siglip2-b16)
        #[arg(long)]
        model: Option<String>,
    },
}

#[derive(Debug, Clone, ValueEnum, PartialEq, Eq)]
pub enum ScanBy {
    Title,
    Filename,
    Datetaken,
}

impl std::fmt::Display for ScanBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanBy::Title => write!(f, "title"),
            ScanBy::Filename => write!(f, "filename"),
            ScanBy::Datetaken => write!(f, "datetaken"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // --- Auth ---

    #[test]
    fn test_parse_auth() {
        let cli = Cli::try_parse_from(["flickr-cli", "auth"]).unwrap();
        assert!(matches!(cli.command, Commands::Auth));
    }

    // --- Scan ---

    #[test]
    fn test_parse_scan_default_by() {
        let cli = Cli::try_parse_from(["flickr-cli", "scan"]).unwrap();
        if let Commands::Scan { by } = cli.command {
            assert!(by.contains(&ScanBy::Title));
            assert!(by.contains(&ScanBy::Filename));
        } else {
            panic!("Expected Scan command");
        }
    }

    #[test]
    fn test_parse_scan_custom_by() {
        let cli = Cli::try_parse_from(["flickr-cli", "scan", "--by", "title,datetaken"]).unwrap();
        if let Commands::Scan { by } = cli.command {
            assert!(by.contains(&ScanBy::Title));
            assert!(by.contains(&ScanBy::Datetaken));
            assert!(!by.contains(&ScanBy::Filename));
        } else {
            panic!("Expected Scan command");
        }
    }

    // --- FuzzyScan ---

    #[test]
    fn test_parse_fuzzy_scan_default_threshold() {
        let cli = Cli::try_parse_from(["flickr-cli", "fuzzy-scan"]).unwrap();
        if let Commands::FuzzyScan { threshold } = cli.command {
            assert_eq!(threshold, 85);
        } else {
            panic!("Expected FuzzyScan command");
        }
    }

    #[test]
    fn test_parse_fuzzy_scan_custom_threshold() {
        let cli = Cli::try_parse_from(["flickr-cli", "fuzzy-scan", "--threshold", "90"]).unwrap();
        if let Commands::FuzzyScan { threshold } = cli.command {
            assert_eq!(threshold, 90);
        } else {
            panic!("Expected FuzzyScan command");
        }
    }

    // --- SyncFlickr ---

    #[test]
    fn test_parse_sync_flickr() {
        let cli = Cli::try_parse_from(["flickr-cli", "sync-flickr", "--directory", "/tmp/photos"])
            .unwrap();
        if let Commands::SyncFlickr {
            directory,
            max_images,
        } = cli.command
        {
            assert_eq!(directory, "/tmp/photos");
            assert_eq!(max_images, None);
        } else {
            panic!("Expected SyncFlickr command");
        }
    }

    #[test]
    fn test_parse_sync_flickr_with_max_images() {
        let cli = Cli::try_parse_from([
            "flickr-cli",
            "sync-flickr",
            "--directory",
            "/tmp",
            "--max-images",
            "100",
        ])
        .unwrap();
        if let Commands::SyncFlickr {
            directory,
            max_images,
        } = cli.command
        {
            assert_eq!(directory, "/tmp");
            assert_eq!(max_images, Some(100));
        } else {
            panic!("Expected SyncFlickr command");
        }
    }

    #[test]
    fn test_parse_sync_flickr_missing_directory_is_err() {
        let result = Cli::try_parse_from(["flickr-cli", "sync-flickr"]);
        assert!(result.is_err());
    }

    // --- AiDedupe ---

    #[test]
    fn test_parse_ai_dedupe_defaults() {
        let cli = Cli::try_parse_from(["flickr-cli", "ai-dedupe", "--directory", "/tmp"]).unwrap();
        if let Commands::AiDedupe {
            directory,
            method,
            model,
            max_images,
            similarity_threshold,
        } = cli.command
        {
            assert_eq!(directory, "/tmp");
            assert_eq!(method, None);
            assert_eq!(model, None);
            assert_eq!(max_images, None);
            assert!((similarity_threshold - 0.95).abs() < 1e-6);
        } else {
            panic!("Expected AiDedupe command");
        }
    }

    #[test]
    fn test_parse_ai_dedupe_all_options() {
        let cli = Cli::try_parse_from([
            "flickr-cli",
            "ai-dedupe",
            "--directory",
            "/tmp",
            "--method",
            "phash",
            "--model",
            "siglip2-b16",
            "--max-images",
            "50",
            "--similarity-threshold",
            "0.9",
        ])
        .unwrap();
        if let Commands::AiDedupe {
            directory,
            method,
            model,
            max_images,
            similarity_threshold,
        } = cli.command
        {
            assert_eq!(directory, "/tmp");
            assert_eq!(method, Some("phash".to_string()));
            assert_eq!(model, Some("siglip2-b16".to_string()));
            assert_eq!(max_images, Some(50));
            assert!((similarity_threshold - 0.9).abs() < 1e-6);
        } else {
            panic!("Expected AiDedupe command");
        }
    }

    #[test]
    fn test_parse_ai_dedupe_missing_directory_is_err() {
        let result = Cli::try_parse_from(["flickr-cli", "ai-dedupe"]);
        assert!(result.is_err());
    }

    // --- BenchmarkMethods ---

    #[test]
    fn test_parse_benchmark_methods_defaults() {
        let cli = Cli::try_parse_from(["flickr-cli", "benchmark-methods", "--directory", "/tmp"])
            .unwrap();
        if let Commands::BenchmarkMethods {
            directory,
            num_images,
            model,
        } = cli.command
        {
            assert_eq!(directory, "/tmp");
            assert_eq!(num_images, 10);
            assert_eq!(model, None);
        } else {
            panic!("Expected BenchmarkMethods command");
        }
    }

    #[test]
    fn test_parse_benchmark_methods_with_options() {
        let cli = Cli::try_parse_from([
            "flickr-cli",
            "benchmark-methods",
            "--directory",
            "/tmp",
            "--num-images",
            "20",
            "--model",
            "clip-vit-b32",
        ])
        .unwrap();
        if let Commands::BenchmarkMethods {
            directory,
            num_images,
            model,
        } = cli.command
        {
            assert_eq!(directory, "/tmp");
            assert_eq!(num_images, 20);
            assert_eq!(model, Some("clip-vit-b32".to_string()));
        } else {
            panic!("Expected BenchmarkMethods command");
        }
    }

    // --- Missing / no subcommand errors ---

    #[test]
    fn test_no_subcommand_is_err() {
        let result = Cli::try_parse_from(["flickr-cli"]);
        assert!(result.is_err());
    }

    // --- ScanBy Display ---

    #[test]
    fn test_scan_by_display_title() {
        assert_eq!(ScanBy::Title.to_string(), "title");
    }

    #[test]
    fn test_scan_by_display_filename() {
        assert_eq!(ScanBy::Filename.to_string(), "filename");
    }

    #[test]
    fn test_scan_by_display_datetaken() {
        assert_eq!(ScanBy::Datetaken.to_string(), "datetaken");
    }
}
