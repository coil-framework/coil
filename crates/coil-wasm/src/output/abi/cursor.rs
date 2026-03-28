use crate::error::WasmModelError;

pub(super) fn write_u8(bytes: &mut Vec<u8>, value: u8) {
    bytes.push(value);
}

pub(super) fn write_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend(value.to_le_bytes());
}

pub(super) fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

pub(super) fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_le_bytes());
}

pub(super) fn write_string(bytes: &mut Vec<u8>, value: &str) -> Result<(), WasmModelError> {
    write_u32(bytes, write_len_u32("string_length", value.len())?);
    bytes.extend(value.as_bytes());
    Ok(())
}

pub(super) fn write_option_string(
    bytes: &mut Vec<u8>,
    value: Option<&String>,
) -> Result<(), WasmModelError> {
    match value {
        Some(value) => {
            write_u8(bytes, 1);
            write_string(bytes, value)?;
        }
        None => write_u8(bytes, 0),
    }
    Ok(())
}

pub(super) fn write_len_u32(field: &'static str, len: usize) -> Result<u32, WasmModelError> {
    <u32 as std::convert::TryFrom<usize>>::try_from(len).map_err(|_| {
        WasmModelError::InvalidTypedReturn {
            reason: format!("`{field}` exceeds the maximum typed ABI length"),
        }
    })
}

pub(super) fn read_option_string(
    cursor: &mut ByteCursor<'_>,
) -> Result<Option<String>, WasmModelError> {
    Ok(match cursor.read_u8()? {
        0 => None,
        1 => Some(cursor.read_string()?),
        other => {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: format!("typed optional string tag `{other}` is invalid"),
            });
        }
    })
}

#[derive(Debug)]
pub(super) struct ByteCursor<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) offset: usize,
}

impl<'a> ByteCursor<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.offset >= self.bytes.len()
    }

    pub(super) fn read_u8(&mut self) -> Result<u8, WasmModelError> {
        if self.offset >= self.bytes.len() {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "unexpected end of typed return payload".to_string(),
            });
        }
        let value = self.bytes[self.offset];
        self.offset += 1;
        Ok(value)
    }

    pub(super) fn read_array<const N: usize>(&mut self) -> Result<[u8; N], WasmModelError> {
        let Some(end) = self.offset.checked_add(N) else {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "unexpected end of typed return payload".to_string(),
            });
        };
        if end > self.bytes.len() {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "unexpected end of typed return payload".to_string(),
            });
        }
        let mut out = [0u8; N];
        out.copy_from_slice(&self.bytes[self.offset..end]);
        self.offset = end;
        Ok(out)
    }

    pub(super) fn read_u16(&mut self) -> Result<u16, WasmModelError> {
        Ok(u16::from_le_bytes(self.read_array()?))
    }

    pub(super) fn read_u32(&mut self) -> Result<u32, WasmModelError> {
        Ok(u32::from_le_bytes(self.read_array()?))
    }

    pub(super) fn read_u64(&mut self) -> Result<u64, WasmModelError> {
        Ok(u64::from_le_bytes(self.read_array()?))
    }

    pub(super) fn read_string(&mut self) -> Result<String, WasmModelError> {
        let len = self.read_u32()? as usize;
        let Some(end) = self.offset.checked_add(len) else {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "typed return payload string extends past buffer".to_string(),
            });
        };
        if end > self.bytes.len() {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "typed return payload string extends past buffer".to_string(),
            });
        }
        let bytes = &self.bytes[self.offset..end];
        self.offset = end;
        let value =
            std::str::from_utf8(bytes).map_err(|error| WasmModelError::InvalidTypedReturn {
                reason: error.to_string(),
            })?;
        Ok(value.to_string())
    }
}
