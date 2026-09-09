//! The shared wire codec: the `[tag: u8][payload]` record format every
//! byte-carrying payload uses when it travels through the
//! transient-buffer table (async task results, callback arguments and
//! results).
//!
//! ONE tag table serves the whole framework: `bffi-async` and
//! `bffi-callback` quote these constants, and the JS-side decoder
//! (`packages/bffi/src/runtime/wire.ts`) mirrors them. Little-endian
//! everywhere; `i64`/`u64` payloads are exact (no `f64` narrowing);
//! `Str`/`Bytes` payloads carry a `u32` LE length prefix.
//!
//! | Tag | Payload                                   |
//! | --- | ----------------------------------------- |
//! | 0   | `Unit` (no bytes)                         |
//! | 1   | `i32` (4 bytes LE)                        |
//! | 2   | `i64` (8 bytes LE)                        |
//! | 3   | `f64` (8 bytes LE)                        |
//! | 4   | `bool` (1 byte, `0`/`1`)                  |
//! | 5   | UTF-8 string (`u32` LE length + bytes)    |
//! | 6   | raw bytes (`u32` LE length + bytes)       |
//!
//! The module holds no unsafe and never panics: every reader is total
//! (`Option`), every writer is an infallible `Vec` push.

/// The `Unit` record: a single tag byte, no payload.
pub const TAG_UNIT: u8 = 0;
/// The `i32` record.
pub const TAG_I32: u8 = 1;
/// The `i64` record.
pub const TAG_I64: u8 = 2;
/// The `f64` record.
pub const TAG_F64: u8 = 3;
/// The `bool` record.
pub const TAG_BOOL: u8 = 4;
/// The UTF-8 string record (`u32` LE length prefix).
pub const TAG_STR: u8 = 5;
/// The raw-bytes record (`u32` LE length prefix).
pub const TAG_BYTES: u8 = 6;

/// Appends one little-endian `u32`.
pub fn push_u32_le(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Appends one little-endian `i32`.
pub fn push_i32_le(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Appends one little-endian `i64`.
pub fn push_i64_le(out: &mut Vec<u8>, value: i64) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Appends one little-endian `f64`.
pub fn push_f64_le(out: &mut Vec<u8>, value: f64) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Appends one `bool` byte (`0`/`1`).
pub fn push_bool(out: &mut Vec<u8>, value: bool) {
    out.push(u8::from(value));
}

/// Reads one little-endian `u32` at `offset`; `None` past the end.
#[must_use]
pub fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let tail = bytes.get(offset..offset + 4)?;
    let mut raw = [0_u8; 4];
    raw.copy_from_slice(tail);
    Some(u32::from_le_bytes(raw))
}

/// Reads one little-endian `i32` at `offset`; `None` past the end.
#[must_use]
pub fn read_i32_le(bytes: &[u8], offset: usize) -> Option<i32> {
    let tail = bytes.get(offset..offset + 4)?;
    let mut raw = [0_u8; 4];
    raw.copy_from_slice(tail);
    Some(i32::from_le_bytes(raw))
}

/// Reads one little-endian `i64` at `offset`; `None` past the end.
#[must_use]
pub fn read_i64_le(bytes: &[u8], offset: usize) -> Option<i64> {
    let tail = bytes.get(offset..offset + 8)?;
    let mut raw = [0_u8; 8];
    raw.copy_from_slice(tail);
    Some(i64::from_le_bytes(raw))
}

/// Reads one little-endian `f64` at `offset`; `None` past the end.
#[must_use]
pub fn read_f64_le(bytes: &[u8], offset: usize) -> Option<f64> {
    let tail = bytes.get(offset..offset + 8)?;
    let mut raw = [0_u8; 8];
    raw.copy_from_slice(tail);
    Some(f64::from_le_bytes(raw))
}

/// Reads one `bool` byte at `offset` (`0` = false, anything else =
/// true); `None` past the end.
#[must_use]
pub fn read_bool(bytes: &[u8], offset: usize) -> Option<bool> {
    let raw = *bytes.get(offset)?;
    Some(raw != 0)
}

#[cfg(test)]
mod tests {
    use super::{
        TAG_BOOL, TAG_BYTES, TAG_F64, TAG_I32, TAG_I64, TAG_STR, TAG_UNIT, push_bool, push_f64_le,
        push_i32_le, push_i64_le, push_u32_le, read_bool, read_f64_le, read_i32_le, read_i64_le,
        read_u32_le,
    };

    #[test]
    fn tag_table_matches_the_documented_layout() {
        assert_eq!(TAG_UNIT, 0);
        assert_eq!(TAG_I32, 1);
        assert_eq!(TAG_I64, 2);
        assert_eq!(TAG_F64, 3);
        assert_eq!(TAG_BOOL, 4);
        assert_eq!(TAG_STR, 5);
        assert_eq!(TAG_BYTES, 6);
    }

    #[test]
    fn pushes_and_reads_round_trip_little_endian() {
        let mut out = Vec::new();
        push_u32_le(&mut out, 0xDEAD_BEEF_u32);
        push_i32_le(&mut out, -2);
        push_i64_le(&mut out, 1);
        push_f64_le(&mut out, 1.5);
        push_bool(&mut out, true);

        assert_eq!(read_u32_le(&out, 0), Some(0xDEAD_BEEF));
        assert_eq!(read_i32_le(&out, 4), Some(-2));
        assert_eq!(read_i64_le(&out, 8), Some(1));
        assert_eq!(read_f64_le(&out, 16), Some(1.5));
        assert_eq!(read_bool(&out, 24), Some(true));
        assert_eq!(out.len(), 25);
    }

    #[test]
    fn readers_past_the_end_are_none() {
        let mut out = Vec::new();
        push_i32_le(&mut out, 7);
        assert_eq!(read_i32_le(&out, 1), None);
        assert_eq!(read_i64_le(&out, 0), None);
        assert_eq!(read_f64_le(&out, 0), None);
        assert_eq!(read_u32_le(&[], 0), None);
        assert_eq!(read_bool(&[], 0), None);
    }
}
