use std::path::PathBuf;

use image_hasher::{HashAlg, HasherConfig};

use crate::engine::{progress_bar, DuplicatePair};
use crate::DedupeError;

/// Find duplicates using perceptual hashing (dHash).
///
/// This is the zero-dependency fallback that needs no ML model files.
/// dHash compares adjacent pixels' relative brightness, producing a
/// compact fingerprint robust to resizing and minor edits.
pub fn find_duplicates_phash(
    paths: &[PathBuf],
    similarity_threshold: f32,
) -> Result<Vec<DuplicatePair>, DedupeError> {
    let hasher = HasherConfig::new()
        .hash_alg(HashAlg::DoubleGradient)
        .hash_size(16, 16)
        .to_hasher();

    // Compute hashes
    let pb = progress_bar(paths.len() as u64, "Hashing images");
    let mut hashes = Vec::with_capacity(paths.len());

    for path in paths {
        let img = image::open(path)?;
        let hash = hasher.hash_image(&img);
        hashes.push(hash);
        pb.inc(1);
    }
    pb.finish_with_message("Hashing complete");

    // Compare all pairs
    let total = (paths.len() * (paths.len() - 1)) / 2;
    let pb = progress_bar(total as u64, "Comparing hashes");
    let mut pairs = Vec::new();

    for i in 0..hashes.len() {
        for j in (i + 1)..hashes.len() {
            let dist = hashes[i].dist(&hashes[j]);
            // Convert hamming distance to similarity (0.0 to 1.0)
            // Max distance for 16x16 dHash = 256 bits
            let max_bits = 256u32;
            let similarity = 1.0 - (dist as f32 / max_bits as f32);

            if similarity >= similarity_threshold {
                pairs.push(DuplicatePair {
                    path_a: paths[i].clone(),
                    path_b: paths[j].clone(),
                    similarity,
                });
            }
            pb.inc(1);
        }
    }
    pb.finish_with_message("Comparison complete");

    Ok(pairs)
}

#[cfg(feature = "phash")]
#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, RgbImage};
    use tempfile::TempDir;

    fn make_gradient_image(width: u32, height: u32) -> RgbImage {
        ImageBuffer::from_fn(width, height, |x, y| {
            image::Rgb([(x * 4) as u8, (y * 4) as u8, 128])
        })
    }

    fn make_inverted_image(width: u32, height: u32) -> RgbImage {
        ImageBuffer::from_fn(width, height, |x, y| {
            image::Rgb([255 - (x * 4) as u8, (y * 2) as u8, 0])
        })
    }

    fn make_solid_image(width: u32, height: u32, r: u8, g: u8, b: u8) -> RgbImage {
        ImageBuffer::from_fn(width, height, |_x, _y| image::Rgb([r, g, b]))
    }

    #[test]
    fn identical_images_are_duplicates() {
        let dir = TempDir::new().unwrap();
        let img = make_gradient_image(64, 64);
        let path_a = dir.path().join("a.jpg");
        let path_b = dir.path().join("b.jpg");
        img.save(&path_a).unwrap();
        img.save(&path_b).unwrap();

        let paths = vec![path_a, path_b];
        let pairs = find_duplicates_phash(&paths, 0.95).unwrap();
        assert_eq!(
            pairs.len(),
            1,
            "identical images should be found as a duplicate pair"
        );
        // Identical images should have similarity very close to 1.0
        assert!(
            pairs[0].similarity > 0.95,
            "identical images should have similarity > 0.95, got {}",
            pairs[0].similarity
        );
    }

    #[test]
    fn very_different_images_not_duplicates() {
        let dir = TempDir::new().unwrap();
        let gradient = make_gradient_image(64, 64);
        let inverted = make_inverted_image(64, 64);

        let path_a = dir.path().join("gradient.jpg");
        let path_b = dir.path().join("inverted.jpg");
        gradient.save(&path_a).unwrap();
        inverted.save(&path_b).unwrap();

        let paths = vec![path_a, path_b];
        let pairs = find_duplicates_phash(&paths, 0.95).unwrap();
        assert!(
            pairs.is_empty(),
            "visually different images should not be duplicates at threshold 0.95, got {} pairs",
            pairs.len()
        );
    }

    #[test]
    fn single_image_no_pairs() {
        let dir = TempDir::new().unwrap();
        let img = make_gradient_image(64, 64);
        let path = dir.path().join("only.jpg");
        img.save(&path).unwrap();

        let paths = vec![path];
        let pairs = find_duplicates_phash(&paths, 0.5).unwrap();
        assert!(pairs.is_empty(), "single image cannot form any pair");
    }

    #[test]
    fn low_threshold_may_find_near_duplicates() {
        let dir = TempDir::new().unwrap();
        let img = make_gradient_image(64, 64);
        let path_a = dir.path().join("a.jpg");
        let path_b = dir.path().join("b.jpg");
        img.save(&path_a).unwrap();
        img.save(&path_b).unwrap();

        let paths = vec![path_a, path_b];
        // At threshold 0.5, identical images should definitely be found
        let pairs = find_duplicates_phash(&paths, 0.5).unwrap();
        assert!(
            !pairs.is_empty(),
            "identical images should be found at threshold 0.5"
        );
        assert!(pairs[0].similarity >= 0.5);
    }

    #[test]
    fn identical_solid_color_images_are_duplicates() {
        // Solid color images are easy edge cases — all pixels identical
        let dir = TempDir::new().unwrap();
        let img = make_solid_image(64, 64, 100, 150, 200);
        let path_a = dir.path().join("a.png");
        let path_b = dir.path().join("b.png");
        img.save(&path_a).unwrap();
        img.save(&path_b).unwrap();

        let paths = vec![path_a, path_b];
        let pairs = find_duplicates_phash(&paths, 0.95).unwrap();
        assert_eq!(
            pairs.len(),
            1,
            "identical solid-color images should be detected as duplicates"
        );
    }
}
