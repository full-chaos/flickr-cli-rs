use std::path::PathBuf;

use crate::engine::DuplicatePair;

/// Compute pairwise cosine similarities and return pairs above the threshold.
///
/// Assumes embeddings are already L2-normalized, so cosine similarity = dot product.
pub fn find_similar_pairs(
    embeddings: &[Vec<f32>],
    paths: &[PathBuf],
    threshold: f32,
) -> Vec<DuplicatePair> {
    let n = embeddings.len();
    let mut pairs = Vec::new();

    for i in 0..n {
        for j in (i + 1)..n {
            let sim = dot_product(&embeddings[i], &embeddings[j]);
            if sim >= threshold {
                pairs.push(DuplicatePair {
                    path_a: paths[i].clone(),
                    path_b: paths[j].clone(),
                    similarity: sim,
                });
            }
        }
    }

    pairs
}

/// Dot product of two vectors (= cosine similarity when L2-normalized).
fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// L2-normalize a vector in place.
pub fn l2_normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Helper to assert approximate float equality.
    fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    // --- l2_normalize ---

    #[test]
    fn l2_normalize_3_4_5_triangle() {
        let mut v = vec![3.0f32, 4.0];
        l2_normalize(&mut v);
        assert!(approx_eq(v[0], 0.6, 1e-6), "expected 0.6, got {}", v[0]);
        assert!(approx_eq(v[1], 0.8, 1e-6), "expected 0.8, got {}", v[1]);
    }

    #[test]
    fn l2_normalize_zero_vector_stays_zero() {
        let mut v = vec![0.0f32, 0.0, 0.0];
        l2_normalize(&mut v);
        assert_eq!(v, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn l2_normalize_already_unit_vector_unchanged() {
        let mut v = vec![0.6f32, 0.8];
        l2_normalize(&mut v);
        assert!(approx_eq(v[0], 0.6, 1e-6));
        assert!(approx_eq(v[1], 0.8, 1e-6));
    }

    #[test]
    fn l2_normalize_single_element() {
        let mut v = vec![5.0f32];
        l2_normalize(&mut v);
        assert!(approx_eq(v[0], 1.0, 1e-6), "expected 1.0, got {}", v[0]);
    }

    // --- dot_product (tested via find_similar_pairs which calls it) ---
    // We test it indirectly by constructing known embeddings.

    #[test]
    fn dot_product_orthogonal_vectors_via_pairs() {
        // [1, 0] · [0, 1] = 0.0 — orthogonal, should not appear above threshold 0.5
        let embeddings = vec![vec![1.0f32, 0.0], vec![0.0f32, 1.0]];
        let paths = vec![PathBuf::from("a.jpg"), PathBuf::from("b.jpg")];
        let pairs = find_similar_pairs(&embeddings, &paths, 0.5);
        assert!(
            pairs.is_empty(),
            "orthogonal unit vectors should not be similar"
        );
    }

    #[test]
    fn dot_product_parallel_unit_vectors() {
        // [1, 0] · [1, 0] = 1.0 — identical, threshold 0.99
        let embeddings = vec![vec![1.0f32, 0.0], vec![1.0f32, 0.0]];
        let paths = vec![PathBuf::from("a.jpg"), PathBuf::from("b.jpg")];
        let pairs = find_similar_pairs(&embeddings, &paths, 0.99);
        assert_eq!(pairs.len(), 1);
        assert!(approx_eq(pairs[0].similarity, 1.0, 1e-6));
    }

    #[test]
    fn dot_product_anti_parallel_unit_vectors() {
        // [1, 0] · [-1, 0] = -1.0 — threshold 0.0 should NOT include it (sim = -1.0 < 0.0)
        let embeddings = vec![vec![1.0f32, 0.0], vec![-1.0f32, 0.0]];
        let paths = vec![PathBuf::from("a.jpg"), PathBuf::from("b.jpg")];
        let pairs = find_similar_pairs(&embeddings, &paths, 0.0);
        // sim = -1.0, which is NOT >= 0.0
        assert!(
            pairs.is_empty(),
            "anti-parallel similarity is -1.0, below threshold 0.0"
        );
    }

    #[test]
    fn dot_product_known_values() {
        // [1, 2, 3] · [4, 5, 6] = 4 + 10 + 18 = 32
        // To use via find_similar_pairs we need normalized vectors; test raw calculation
        // by checking that a normalized version produces the right cosine.
        // [1,2,3] norm = sqrt(14), [4,5,6] norm = sqrt(77)
        // cos = 32 / (sqrt(14)*sqrt(77)) = 32 / sqrt(1078) ≈ 0.9746
        let mut a = vec![1.0f32, 2.0, 3.0];
        let mut b = vec![4.0f32, 5.0, 6.0];
        l2_normalize(&mut a);
        l2_normalize(&mut b);
        let paths = vec![PathBuf::from("a.jpg"), PathBuf::from("b.jpg")];
        let pairs = find_similar_pairs(&[a, b], &paths, 0.97);
        assert_eq!(pairs.len(), 1);
        assert!(
            approx_eq(pairs[0].similarity, 0.9746318, 1e-4),
            "got {}",
            pairs[0].similarity
        );
    }

    // --- find_similar_pairs ---

    #[test]
    fn find_similar_pairs_identical_embeddings_returns_pair() {
        let mut emb = vec![1.0f32, 0.0, 0.0];
        l2_normalize(&mut emb);
        let embeddings = vec![emb.clone(), emb.clone()];
        let paths = vec![PathBuf::from("a.jpg"), PathBuf::from("b.jpg")];
        let pairs = find_similar_pairs(&embeddings, &paths, 0.99);
        assert_eq!(pairs.len(), 1);
        assert!(approx_eq(pairs[0].similarity, 1.0, 1e-6));
    }

    #[test]
    fn find_similar_pairs_orthogonal_below_threshold() {
        let embeddings = vec![vec![1.0f32, 0.0], vec![0.0f32, 1.0]];
        let paths = vec![PathBuf::from("a.jpg"), PathBuf::from("b.jpg")];
        let pairs = find_similar_pairs(&embeddings, &paths, 0.5);
        assert!(pairs.is_empty());
    }

    #[test]
    fn find_similar_pairs_three_embeddings_only_two_similar() {
        // a and b are identical; c is orthogonal to both
        let a = vec![1.0f32, 0.0];
        let b = vec![1.0f32, 0.0];
        let c = vec![0.0f32, 1.0];
        let embeddings = vec![a, b, c];
        let paths = vec![
            PathBuf::from("a.jpg"),
            PathBuf::from("b.jpg"),
            PathBuf::from("c.jpg"),
        ];
        let pairs = find_similar_pairs(&embeddings, &paths, 0.99);
        assert_eq!(pairs.len(), 1, "only a-b pair should be returned");
        assert_eq!(pairs[0].path_a, PathBuf::from("a.jpg"));
        assert_eq!(pairs[0].path_b, PathBuf::from("b.jpg"));
    }

    #[test]
    fn find_similar_pairs_empty_embeddings_returns_empty() {
        let pairs = find_similar_pairs(&[], &[], 0.5);
        assert!(pairs.is_empty());
    }

    #[test]
    fn find_similar_pairs_threshold_zero_returns_all_non_negative_pairs() {
        // With threshold 0.0, any pair with sim >= 0.0 is returned.
        // [1,0] · [0,1] = 0.0, so it IS included (>= 0.0).
        let embeddings = vec![vec![1.0f32, 0.0], vec![0.0f32, 1.0]];
        let paths = vec![PathBuf::from("a.jpg"), PathBuf::from("b.jpg")];
        let pairs = find_similar_pairs(&embeddings, &paths, 0.0);
        // dot = 0.0 which equals threshold 0.0
        assert_eq!(pairs.len(), 1);
        assert!(approx_eq(pairs[0].similarity, 0.0, 1e-6));
    }

    #[test]
    fn find_similar_pairs_threshold_one_only_exact_matches() {
        let identical = vec![1.0f32, 0.0];
        let near = vec![0.9999f32, 0.01f32]; // not exactly unit, but close
        let embeddings = vec![identical.clone(), identical.clone(), near];
        let paths = vec![
            PathBuf::from("a.jpg"),
            PathBuf::from("b.jpg"),
            PathBuf::from("c.jpg"),
        ];
        let pairs = find_similar_pairs(&embeddings, &paths, 1.0);
        // Only the a-b pair has similarity exactly 1.0
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].path_a, PathBuf::from("a.jpg"));
        assert_eq!(pairs[0].path_b, PathBuf::from("b.jpg"));
    }
}
