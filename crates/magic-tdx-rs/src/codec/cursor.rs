use crate::TdxError;

/// Bounds-checked byte cursor.
#[derive(Debug)]
pub struct ByteCursor<'a> {
    operation: &'a str,
    input: &'a [u8],
    position: usize,
}

impl<'a> ByteCursor<'a> {
    /// Creates a cursor at the beginning of `input`.
    pub fn new(operation: &'a str, input: &'a [u8]) -> Self {
        Self {
            operation,
            input,
            position: 0,
        }
    }

    fn take(&mut self, count: usize, field: &str) -> Result<&'a [u8], TdxError> {
        let end = self.position.checked_add(count).ok_or_else(|| {
            TdxError::InvalidData(format!(
                "TDX {} field {field} offset {} overflows",
                self.operation, self.position
            ))
        })?;
        if end > self.input.len() {
            return Err(TdxError::InvalidData(format!(
                "TDX {} field {field} needs {count} bytes at offset {}, input length is {}",
                self.operation,
                self.position,
                self.input.len()
            )));
        }
        let output = &self.input[self.position..end];
        self.position = end;
        Ok(output)
    }

    /// Reads one byte.
    pub fn read_u8(&mut self, field: &str) -> Result<u8, TdxError> {
        Ok(self.take(1, field)?[0])
    }

    /// Reads one little-endian `u32`.
    pub fn read_u32_le(&mut self, field: &str) -> Result<u32, TdxError> {
        let bytes = self.take(4, field)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Returns the current byte offset.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Returns the unread byte count.
    pub fn remaining(&self) -> usize {
        self.input.len() - self.position
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_values_and_tracks_position() {
        let mut cursor = ByteCursor::new("quote", &[7, 1, 2, 3, 4, 9]);
        assert_eq!(cursor.position(), 0);
        assert_eq!(cursor.remaining(), 6);
        assert_eq!(cursor.read_u8("tag").unwrap(), 7);
        assert_eq!(cursor.read_u32_le("value").unwrap(), 0x0403_0201);
        assert_eq!(cursor.position(), 5);
        assert_eq!(cursor.remaining(), 1);
        assert_eq!(cursor.read_u8("tail").unwrap(), 9);
        assert_eq!(cursor.remaining(), 0);
    }

    #[test]
    fn truncated_fields_preserve_operation_field_and_offset() {
        let mut cursor = ByteCursor::new("quote", &[1, 2, 3]);
        let error = cursor.read_u32_le("price").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("quote"));
        assert!(message.contains("price"));
        assert!(message.contains("offset 0"));
        assert_eq!(cursor.position(), 0);
    }

    #[test]
    fn offset_overflow_is_explicit() {
        let mut cursor = ByteCursor {
            operation: "quote",
            input: &[],
            position: usize::MAX,
        };
        let error = cursor.take(1, "price").unwrap_err();
        assert!(error.to_string().contains("overflows"));
    }
}
