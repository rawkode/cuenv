use base64::{engine::general_purpose::STANDARD, Engine};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

#[derive(Debug, thiserror::Error)]
pub enum GzenvError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Compression error: {0}")]
    Compression(#[from] std::io::Error),

    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
}

/// Encode data to gzenv format (JSON -> gzip -> base64)
pub fn encode<T: Serialize>(data: &T) -> Result<String, GzenvError> {
    // Convert to JSON
    let json = serde_json::to_vec(data)?;

    // Compress with gzip
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&json)?;
    let compressed = encoder.finish()?;

    // Encode to base64
    Ok(STANDARD.encode(compressed))
}

/// Decode data from gzenv format (base64 -> gzip -> JSON)
pub fn decode<T: for<'de> Deserialize<'de>>(encoded: &str) -> Result<T, GzenvError> {
    // Decode from base64
    let compressed = STANDARD.decode(encoded)?;

    // Decompress gzip
    let mut decoder = GzDecoder::new(&compressed[..]);
    let mut json = Vec::new();
    decoder.read_to_end(&mut json)?;

    // Parse JSON
    Ok(serde_json::from_slice(&json)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_encode_decode_hashmap() {
        let mut data = HashMap::new();
        data.insert("FOO".to_string(), "bar".to_string());
        data.insert("HELLO".to_string(), "world".to_string());

        let encoded = encode(&data).unwrap();
        assert!(!encoded.is_empty());

        let decoded: HashMap<String, String> = decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_encode_decode_struct() {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct TestData {
            name: String,
            value: i32,
            items: Vec<String>,
        }

        let data = TestData {
            name: "test".to_string(),
            value: 42,
            items: vec!["one".to_string(), "two".to_string()],
        };

        let encoded = encode(&data).unwrap();
        let decoded: TestData = decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
}
