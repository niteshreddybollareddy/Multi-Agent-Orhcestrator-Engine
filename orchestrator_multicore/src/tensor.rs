/// Computes the dot product of two f32 slices.
/// Uses standard iterators which LLVM auto-vectorizes to NEON instructions on aarch64.
pub fn simd_dot_product(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "Vectors must be of the same length");
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Cosine similarity using the dot product.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot = simd_dot_product(a, b);
    let norm_a = simd_dot_product(a, a).sqrt();
    let norm_b = simd_dot_product(b, b).sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_dot_product() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let b = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        assert_eq!(simd_dot_product(&a, &b), 45.0);
    }
}
