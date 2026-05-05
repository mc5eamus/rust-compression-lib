use std::fmt;
use std::io;

use rayon::prelude::*;

/// Default compression level used when none is specified.
pub const DEFAULT_COMPRESSION_LEVEL: i32 = 3;

/// Default chunk size for parallel compression (1 MB).
pub const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024;

/// Error type returned by compression and decompression operations.
#[derive(Debug)]
pub struct CompressionError(io::Error);

impl fmt::Display for CompressionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "compression error: {}", self.0)
    }
}

impl std::error::Error for CompressionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

impl From<io::Error> for CompressionError {
    fn from(err: io::Error) -> Self {
        CompressionError(err)
    }
}

/// Compresses the input data using the default compression level (3).
///
/// Uses `zstd::stream::encode_all` for zero-copy streaming operations.
pub fn compress(data: &[u8]) -> Result<Vec<u8>, CompressionError> {
    compress_with_level(data, DEFAULT_COMPRESSION_LEVEL)
}

/// Compresses the input data at the specified compression level.
///
/// Uses `zstd::stream::encode_all` for zero-copy streaming operations.
/// The `level` parameter controls the compression ratio vs. speed tradeoff.
pub fn compress_with_level(data: &[u8], level: i32) -> Result<Vec<u8>, CompressionError> {
    zstd::stream::encode_all(data, level).map_err(CompressionError::from)
}

/// Decompresses zstd-compressed data.
///
/// Uses `zstd::stream::decode_all` for zero-copy streaming operations.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, CompressionError> {
    zstd::stream::decode_all(data).map_err(CompressionError::from)
}

/// Compresses data in parallel by splitting it into chunks.
///
/// Each chunk is compressed independently using rayon's thread pool.
/// Uses the default compression level (3) and chunk size (1 MB).
///
/// The output format is:
/// - 4 bytes: number of chunks (little-endian u32)
/// - For each chunk: 4 bytes compressed length (little-endian u32) + compressed data
pub fn parallel_compress(data: &[u8]) -> Result<Vec<u8>, CompressionError> {
    parallel_compress_with_options(data, DEFAULT_COMPRESSION_LEVEL, DEFAULT_CHUNK_SIZE)
}

/// Compresses data in parallel with configurable level and chunk size.
///
/// Splits `data` into chunks of `chunk_size` bytes, compresses each in parallel,
/// then concatenates them with a header for later parallel decompression.
pub fn parallel_compress_with_options(
    data: &[u8],
    level: i32,
    chunk_size: usize,
) -> Result<Vec<u8>, CompressionError> {
    let chunks: Vec<&[u8]> = data.chunks(chunk_size.max(1)).collect();

    let compressed_chunks: Result<Vec<Vec<u8>>, CompressionError> = chunks
        .par_iter()
        .map(|chunk| compress_with_level(chunk, level))
        .collect();
    let compressed_chunks = compressed_chunks?;

    // Header: chunk count (u32 LE) + per-chunk: length (u32 LE) + data
    let total_size = 4 + compressed_chunks.iter().map(|c| 4 + c.len()).sum::<usize>();
    let mut output = Vec::with_capacity(total_size);
    output.extend_from_slice(&(compressed_chunks.len() as u32).to_le_bytes());
    for chunk in &compressed_chunks {
        output.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        output.extend_from_slice(chunk);
    }

    Ok(output)
}

/// Decompresses data that was compressed with `parallel_compress`.
///
/// Each chunk is decompressed independently in parallel using rayon.
pub fn parallel_decompress(data: &[u8]) -> Result<Vec<u8>, CompressionError> {
    if data.len() < 4 {
        return Err(CompressionError(io::Error::new(
            io::ErrorKind::InvalidData,
            "parallel compressed data too short for header",
        )));
    }

    let chunk_count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut offset = 4;

    // Parse chunk boundaries
    let mut chunk_slices = Vec::with_capacity(chunk_count);
    for _ in 0..chunk_count {
        if offset + 4 > data.len() {
            return Err(CompressionError(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected end of parallel compressed data",
            )));
        }
        let len = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;
        if offset + len > data.len() {
            return Err(CompressionError(io::Error::new(
                io::ErrorKind::InvalidData,
                "chunk length exceeds available data",
            )));
        }
        chunk_slices.push(&data[offset..offset + len]);
        offset += len;
    }

    // Decompress all chunks in parallel
    let decompressed_chunks: Result<Vec<Vec<u8>>, CompressionError> = chunk_slices
        .par_iter()
        .map(|chunk| decompress(chunk))
        .collect();
    let decompressed_chunks = decompressed_chunks?;

    let total_size: usize = decompressed_chunks.iter().map(|c| c.len()).sum();
    let mut output = Vec::with_capacity(total_size);
    for chunk in decompressed_chunks {
        output.extend_from_slice(&chunk);
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_level() {
        assert_eq!(DEFAULT_COMPRESSION_LEVEL, 3);
    }

    #[test]
    fn test_round_trip_empty() {
        let original: &[u8] = b"";
        let compressed = compress(original).expect("compress failed");
        let decompressed = decompress(&compressed).expect("decompress failed");
        assert_eq!(original, decompressed.as_slice());
    }

    #[test]
    fn test_round_trip_small() {
        let original = b"Hello, zstd compression library!";
        let compressed = compress(original).expect("compress failed");
        let decompressed = decompress(&compressed).expect("decompress failed");
        assert_eq!(original.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_round_trip_large() {
        let pattern = b"abcdefghijklmnopqrstuvwxyz0123456789";
        let original: Vec<u8> = pattern.iter().copied().cycle().take(1_000_000).collect();
        let compressed = compress(&original).expect("compress failed");
        let decompressed = decompress(&compressed).expect("decompress failed");
        assert_eq!(original, decompressed);
    }

    #[test]
    fn test_round_trip_all_levels() {
        let original = b"Round-trip at every supported compression level.";
        for level in zstd::compression_level_range() {
            let compressed =
                compress_with_level(original, level).expect("compress failed");
            let decompressed = decompress(&compressed).expect("decompress failed");
            assert_eq!(
                original.as_slice(),
                decompressed.as_slice(),
                "round-trip failed at level {level}"
            );
        }
    }

    #[test]
    fn test_decompress_invalid() {
        let garbage = b"this is not valid zstd data";
        let result = decompress(garbage);
        assert!(result.is_err(), "expected error on invalid input");
    }

    #[test]
    fn test_parallel_round_trip_empty() {
        let original: &[u8] = b"";
        let compressed = parallel_compress(original).expect("parallel compress failed");
        let decompressed = parallel_decompress(&compressed).expect("parallel decompress failed");
        assert_eq!(original, decompressed.as_slice());
    }

    #[test]
    fn test_parallel_round_trip_small() {
        let original = b"Hello, parallel zstd compression!";
        let compressed = parallel_compress(original).expect("parallel compress failed");
        let decompressed = parallel_decompress(&compressed).expect("parallel decompress failed");
        assert_eq!(original.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_parallel_round_trip_large() {
        let pattern = b"abcdefghijklmnopqrstuvwxyz0123456789";
        let original: Vec<u8> = pattern.iter().copied().cycle().take(5_000_000).collect();
        let compressed = parallel_compress(&original).expect("parallel compress failed");
        let decompressed = parallel_decompress(&compressed).expect("parallel decompress failed");
        assert_eq!(original, decompressed);
    }

    #[test]
    fn test_parallel_round_trip_custom_chunk_size() {
        let pattern = b"abcdefghijklmnopqrstuvwxyz0123456789";
        let original: Vec<u8> = pattern.iter().copied().cycle().take(1_000_000).collect();
        for chunk_size in [256 * 1024, 512 * 1024, 2 * 1024 * 1024] {
            let compressed = parallel_compress_with_options(&original, 3, chunk_size)
                .expect("parallel compress failed");
            let decompressed =
                parallel_decompress(&compressed).expect("parallel decompress failed");
            assert_eq!(original, decompressed, "failed with chunk_size {chunk_size}");
        }
    }

    #[test]
    fn test_parallel_decompress_invalid() {
        let garbage = b"xx";
        let result = parallel_decompress(garbage);
        assert!(result.is_err(), "expected error on invalid parallel input");
    }
}
