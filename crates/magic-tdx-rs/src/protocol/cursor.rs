use crate::error::Result;
use crate::error_codes::ErrorCode;

const MAX_TDX_VARINT_BYTES: usize = 9;

/// Checked reader for one TDX response packet.
///
/// The cursor never substitutes a default value for missing bytes. Every
/// failure includes the packet offset and the field being decoded.
#[derive(Debug, Clone)]
pub(crate) struct PacketCursor<'a> {
    data: &'a [u8],
    position: usize,
    record: Option<usize>,
}

impl<'a> PacketCursor<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            position: 0,
            record: None,
        }
    }

    pub(crate) fn at(data: &'a [u8], position: usize) -> Result<Self> {
        if position > data.len() {
            return Err(length_error(position, 0, data.len(), "cursor start", None));
        }
        Ok(Self {
            data,
            position,
            record: None,
        })
    }

    pub(crate) fn set_record(&mut self, record: usize) {
        self.record = Some(record);
    }

    pub(crate) fn position(&self) -> usize {
        self.position
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len() - self.position
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub(crate) fn read_slice(&mut self, width: usize, field: &str) -> Result<&'a [u8]> {
        let end = self.position.checked_add(width).ok_or_else(|| {
            length_error(self.position, width, self.data.len(), field, self.record)
        })?;
        let value = self.data.get(self.position..end).ok_or_else(|| {
            length_error(self.position, width, self.data.len(), field, self.record)
        })?;
        self.position = end;
        Ok(value)
    }

    pub(crate) fn read_u8(&mut self, field: &str) -> Result<u8> {
        Ok(self.read_slice(1, field)?[0])
    }

    pub(crate) fn read_u16_le(&mut self, field: &str) -> Result<u16> {
        let bytes: [u8; 2] = self
            .read_slice(2, field)?
            .try_into()
            .map_err(|_| length_error(self.position, 2, self.data.len(), field, self.record))?;
        Ok(u16::from_le_bytes(bytes))
    }

    pub(crate) fn read_u32_le(&mut self, field: &str) -> Result<u32> {
        let bytes: [u8; 4] = self
            .read_slice(4, field)?
            .try_into()
            .map_err(|_| length_error(self.position, 4, self.data.len(), field, self.record))?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub(crate) fn read_i32_le(&mut self, field: &str) -> Result<i32> {
        let bytes: [u8; 4] = self
            .read_slice(4, field)?
            .try_into()
            .map_err(|_| length_error(self.position, 4, self.data.len(), field, self.record))?;
        Ok(i32::from_le_bytes(bytes))
    }

    pub(crate) fn read_i64_le(&mut self, field: &str) -> Result<i64> {
        let bytes: [u8; 8] = self
            .read_slice(8, field)?
            .try_into()
            .map_err(|_| length_error(self.position, 8, self.data.len(), field, self.record))?;
        Ok(i64::from_le_bytes(bytes))
    }

    pub(crate) fn read_f32_le(&mut self, field: &str) -> Result<f32> {
        let bytes: [u8; 4] = self
            .read_slice(4, field)?
            .try_into()
            .map_err(|_| length_error(self.position, 4, self.data.len(), field, self.record))?;
        Ok(f32::from_le_bytes(bytes))
    }

    pub(crate) fn read_tdx_varint(&mut self, field: &str) -> Result<i64> {
        let start = self.position;
        let first = self.read_u8(field)?;
        let negative = first & 0x40 != 0;
        let mut magnitude = u128::from(first & 0x3f);
        let mut shift = 6_u32;
        let mut encoded_len = 1_usize;
        let mut current = first;

        while current & 0x80 != 0 {
            if encoded_len == MAX_TDX_VARINT_BYTES {
                return Err(length_error(
                    start,
                    encoded_len + 1,
                    self.data.len(),
                    field,
                    self.record,
                ));
            }
            current = self.read_u8(field)?;
            magnitude |= u128::from(current & 0x7f) << shift;
            shift += 7;
            encoded_len += 1;
        }

        let magnitude = i64::try_from(magnitude).map_err(|_| {
            ErrorCode::TYPE_MISMATCH.err(format!(
                "{} at offset {start} exceeds the signed 64-bit domain{}",
                field,
                record_suffix(self.record)
            ))
        })?;
        Ok(if negative { -magnitude } else { magnitude })
    }
}

fn record_suffix(record: Option<usize>) -> String {
    record.map_or_else(String::new, |index| format!(" in record {index}"))
}

fn length_error(
    offset: usize,
    requested: usize,
    available: usize,
    field: &str,
    record: Option<usize>,
) -> crate::TdxError {
    ErrorCode::RESPONSE_LENGTH_MISMATCH.err(format!(
        "{field} at offset {offset} requires {requested} byte(s), packet length is {available}{}",
        record_suffix(record)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_reads_report_field_and_offset() {
        let mut cursor = PacketCursor::at(&[1], 1).unwrap();
        let error = cursor.read_u16_le("record volume").unwrap_err();
        assert_eq!(
            error.error_code(),
            Some(ErrorCode::RESPONSE_LENGTH_MISMATCH)
        );
        assert!(error.to_string().contains("record volume at offset 1"));
    }

    #[test]
    fn bounded_slice_never_reads_past_packet() {
        let mut cursor = PacketCursor::new(&[1, 2]);
        assert_eq!(cursor.read_slice(2, "payload").unwrap(), &[1, 2]);
        assert!(cursor.is_empty());
        assert!(cursor.read_u8("tail").is_err());
    }

    #[test]
    fn valid_zero_is_not_a_decoder_failure() {
        let mut cursor = PacketCursor::new(&[0]);
        assert_eq!(cursor.read_tdx_varint("price").unwrap(), 0);
        assert_eq!(cursor.position(), 1);
    }

    #[test]
    fn unterminated_and_overlong_varints_are_rejected() {
        for length in 1..=MAX_TDX_VARINT_BYTES {
            let bytes = vec![0x80; length];
            let mut cursor = PacketCursor::new(&bytes);
            let error = cursor.read_tdx_varint("price").unwrap_err();
            assert_eq!(
                error.error_code(),
                Some(ErrorCode::RESPONSE_LENGTH_MISMATCH)
            );
        }
    }
}
