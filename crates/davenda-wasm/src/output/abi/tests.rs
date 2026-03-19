use super::cursor::{ByteCursor, write_len_u32};
use super::*;

#[test]
fn write_len_u32_rejects_lengths_that_do_not_fit_in_the_abi() {
    let error = write_len_u32("json_object_entry_count", (u32::MAX as usize) + 1).unwrap_err();
    assert_eq!(
        error,
        WasmModelError::InvalidTypedReturn {
            reason: "`json_object_entry_count` exceeds the maximum typed ABI length".to_string(),
        }
    );
}

#[test]
fn byte_cursor_rejects_offset_overflow_when_reading_arrays() {
    let mut cursor = ByteCursor {
        bytes: &[],
        offset: usize::MAX,
    };

    let error = cursor.read_array::<1>().unwrap_err();
    assert_eq!(
        error,
        WasmModelError::InvalidTypedReturn {
            reason: "unexpected end of typed return payload".to_string(),
        }
    );
}
