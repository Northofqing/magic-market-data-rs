//! Bounds-checked helpers for protocol decoding.

mod cursor;

pub use cursor::ByteCursor;

use crate::TdxError;

/// Limits applied while decoding potentially compressed input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    max_decompressed_bytes: usize,
}

impl Limits {
    /// Starts a builder with a 16 MiB decompressed-output limit.
    pub fn builder() -> LimitsBuilder {
        LimitsBuilder {
            max: 16 * 1024 * 1024,
        }
    }

    /// Returns the maximum accepted decompressed output size.
    pub fn max_decompressed_bytes(&self) -> usize {
        self.max_decompressed_bytes
    }
}

/// Builder for [`Limits`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitsBuilder {
    max: usize,
}

impl LimitsBuilder {
    /// Sets the maximum accepted decompressed output size.
    pub fn max_decompressed_bytes(mut self, value: usize) -> Self {
        self.max = value;
        self
    }

    /// Validates and creates the limits.
    pub fn build(self) -> Result<Limits, TdxError> {
        if self.max == 0 {
            return Err(TdxError::InvalidData(
                "TDX decompressed output limit must be positive".into(),
            ));
        }
        Ok(Limits {
            max_decompressed_bytes: self.max,
        })
    }
}

/// Applies the configured output bound to a passthrough payload.
///
/// This helper deliberately does not claim to inflate zlib data; callers that
/// perform decompression use the networking codec and can apply this bound to
/// the resulting bytes.
pub fn decompress_zlib(input: &[u8], limits: &Limits) -> Result<Vec<u8>, TdxError> {
    if input.len() > limits.max_decompressed_bytes() {
        return Err(TdxError::InvalidData(format!(
            "TDX decompressed output length {} exceeds limit {}",
            input.len(),
            limits.max_decompressed_bytes()
        )));
    }
    Ok(input.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_custom_limits_are_reported() {
        let defaults = Limits::builder().build().unwrap();
        assert_eq!(defaults.max_decompressed_bytes(), 16 * 1024 * 1024);

        let custom = Limits::builder().max_decompressed_bytes(3).build().unwrap();
        assert_eq!(custom.max_decompressed_bytes(), 3);
    }

    #[test]
    fn zero_limit_is_rejected() {
        let error = Limits::builder()
            .max_decompressed_bytes(0)
            .build()
            .unwrap_err();
        assert!(error.to_string().contains("must be positive"));
    }

    #[test]
    fn passthrough_respects_the_output_limit() {
        let limits = Limits::builder().max_decompressed_bytes(3).build().unwrap();
        assert_eq!(decompress_zlib(&[1, 2, 3], &limits).unwrap(), [1, 2, 3]);

        let error = decompress_zlib(&[1, 2, 3, 4], &limits).unwrap_err();
        assert!(error.to_string().contains("exceeds limit 3"));
    }
}
