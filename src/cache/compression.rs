use anyhow::{Context, Result};
use flate2::write::{GzDecoder, GzEncoder};
use flate2::Compression;
use std::io::Write;
use tracing::debug;

/// Compress data using gzip
pub fn compress_data(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .context("Failed to write data to compressor")?;

    let compressed = encoder.finish().context("Failed to finish compression")?;

    let ratio = if data.is_empty() {
        0.0
    } else {
        (compressed.len() as f64 / data.len() as f64) * 100.0
    };

    debug!(
        "Compressed {} bytes to {} bytes ({:.1}% ratio)",
        data.len(),
        compressed.len(),
        ratio
    );

    Ok(compressed)
}

/// Decompress gzip data
pub fn decompress_data(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(Vec::new());
    decoder
        .write_all(data)
        .context("Failed to write compressed data to decoder")?;

    let decompressed = decoder.finish().context("Failed to finish decompression")?;

    debug!(
        "Decompressed {} bytes to {} bytes",
        data.len(),
        decompressed.len()
    );

    Ok(decompressed)
}

/// Check if compression is worthwhile for given data
pub fn should_compress(data: &[u8]) -> bool {
    // Don't compress if data is too small
    if data.len() < 1024 {
        return false;
    }

    // Simple heuristic: check if data looks like text (has many repeated patterns)
    // by looking at byte distribution
    let mut byte_counts = [0u32; 256];
    for &byte in data.iter().take(1024) {
        byte_counts[byte as usize] += 1;
    }

    // Count unique bytes
    let unique_bytes = byte_counts.iter().filter(|&&count| count > 0).count();

    // If less than 100 unique bytes in first 1KB, likely compressible
    unique_bytes < 100
}

/// Calculate compression ratio
pub fn compression_ratio(original_size: usize, compressed_size: usize) -> f64 {
    if original_size == 0 {
        return 0.0;
    }

    (1.0 - (compressed_size as f64 / original_size as f64)) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress() {
        let original = b"Hello, world! This is a test string that should compress well.";

        let compressed = compress_data(original).unwrap();
        assert!(compressed.len() > 0);

        let decompressed = decompress_data(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_compress_empty() {
        let original = b"";

        let compressed = compress_data(original).unwrap();
        let decompressed = decompress_data(&compressed).unwrap();

        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_compress_large_repetitive() {
        // Create highly repetitive data that should compress well
        let mut original = Vec::new();
        for _ in 0..1000 {
            original.extend_from_slice(b"abcdefghij");
        }

        let compressed = compress_data(&original).unwrap();

        // Should compress significantly
        assert!(compressed.len() < original.len() / 2);

        let decompressed = decompress_data(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_should_compress() {
        // Small data should not be compressed
        assert!(!should_compress(b"small"));

        // Large repetitive data should be compressed
        let mut repetitive = Vec::new();
        for _ in 0..200 {
            repetitive.extend_from_slice(b"repeat");
        }
        assert!(should_compress(&repetitive));

        // Random-like data might not be worth compressing
        let mut random_like = Vec::new();
        for i in 0..1024 {
            random_like.push((i % 256) as u8);
        }
        assert!(!should_compress(&random_like));
    }

    #[test]
    fn test_compression_ratio() {
        assert_eq!(compression_ratio(100, 25), 75.0);
        assert_eq!(compression_ratio(1000, 100), 90.0);
        assert_eq!(compression_ratio(0, 0), 0.0);
    }
}
