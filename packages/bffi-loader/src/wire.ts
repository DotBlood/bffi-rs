/**
 * The wire codec shared by every byte-carrying payload crossing the
 * boundary: `[tag: u8][payload]`, little-endian, one table for the
 * whole framework (the Rust side is `bffi_types::wire`, mirrored by
 * `bffi-async` and the callback ABI; CALLING-CONVENTION.md).
 *
 * - `Str` payloads are UTF-8 with a `u32` length prefix;
 * - `Bytes` payloads are raw bytes with a `u32` length prefix;
 * - every integer payload is LE, `i64`/`u64` without `f64` narrowing.
 */

export const TAG_UNIT = 0;
export const TAG_I32 = 1;
export const TAG_I64 = 2;
export const TAG_F64 = 3;
export const TAG_BOOL = 4;
export const TAG_STR = 5;
export const TAG_BYTES = 6;

/** A decoded wire value. */
export type WireValue =
  | undefined
  | number
  | bigint
  | boolean
  | string
  | Uint8Array;

/**
 * Decodes one `[tag][payload]` record into a JS value. The bytes view
 * MUST be a subarray starting at the record; `Bytes` payloads are
 * copied out (the source buffer is transient by contract).
 */
export function decodeValue(bytes: Uint8Array): WireValue {
  const tag = bytes[0];
  if (tag === undefined) {
    throw new Error("empty wire payload");
  }
  const view = new DataView(
    bytes.buffer,
    bytes.byteOffset,
    bytes.byteLength,
  );
  switch (tag) {
    case TAG_UNIT:
      return undefined;
    case TAG_I32:
      return view.getInt32(1, true);
    case TAG_I64:
      return view.getBigInt64(1, true);
    case TAG_F64:
      return view.getFloat64(1, true);
    case TAG_BOOL:
      return (bytes[1] ?? 0) !== 0;
    case TAG_STR: {
      const len = view.getUint32(1, true);
      return new TextDecoder().decode(bytes.subarray(5, 5 + len));
    }
    case TAG_BYTES: {
      const len = view.getUint32(1, true);
      return bytes.slice(5, 5 + len);
    }
    default:
      throw new Error(`unknown wire value tag: ${tag}`);
  }
}

/**
 * Encodes one JS value into the `[tag][payload]` record. Numbers
 * encode as `I32` when they are integral and fit `i32`, otherwise
 * `F64`; bigints encode as `I64` (exactness preserved).
 */
export function encodeValue(out: number[], value: WireValue): void {
  if (value === undefined) {
    out.push(TAG_UNIT);
  } else if (typeof value === "number") {
    if (Number.isInteger(value) && value >= -2147483648 && value <= 2147483647) {
      out.push(TAG_I32);
      pushI32(out, value);
    } else {
      out.push(TAG_F64);
      pushF64(out, value);
    }
  } else if (typeof value === "bigint") {
    if (value < -9223372036854775808n || value > 9223372036854775807n) {
      throw new Error(`i64 value out of range: ${value}`);
    }
    out.push(TAG_I64);
    pushI64(out, value);
  } else if (typeof value === "boolean") {
    out.push(TAG_BOOL, value ? 1 : 0);
  } else if (typeof value === "string") {
    out.push(TAG_STR);
    const bytes = new TextEncoder().encode(value);
    pushU32(out, bytes.length);
    for (const byte of bytes) {
      out.push(byte);
    }
  } else {
    out.push(TAG_BYTES);
    pushU32(out, value.length);
    for (const byte of value) {
      out.push(byte);
    }
  }
}

/**
 * Encodes an argument list into one record: the concatenation of each
 * value's `[tag][payload]` record, in order. An empty list produces
 * the single-byte Unit record (`[TAG_UNIT]`).
 */
export function encodeArgs(values: readonly WireValue[]): Uint8Array {
  const out: number[] = [];
  if (values.length === 0) {
    out.push(TAG_UNIT);
  }
  for (const value of values) {
    encodeValue(out, value);
  }
  return new Uint8Array(out);
}

function pushI32(out: number[], value: number): void {
  const view = new DataView(new ArrayBuffer(4));
  view.setInt32(0, value, true);
  for (let i = 0; i < 4; i++) {
    out.push(view.getUint8(i));
  }
}

function pushF64(out: number[], value: number): void {
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, value, true);
  for (let i = 0; i < 8; i++) {
    out.push(view.getUint8(i));
  }
}

function pushI64(out: number[], value: bigint): void {
  const view = new DataView(new ArrayBuffer(8));
  view.setBigInt64(0, value, true);
  for (let i = 0; i < 8; i++) {
    out.push(view.getUint8(i));
  }
}

function pushU32(out: number[], value: number): void {
  const view = new DataView(new ArrayBuffer(4));
  view.setUint32(0, value, true);
  for (let i = 0; i < 4; i++) {
    out.push(view.getUint8(i));
  }
}
