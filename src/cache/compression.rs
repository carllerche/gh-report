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
}
