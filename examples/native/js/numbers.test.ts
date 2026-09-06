/**
 * P1 type-matrix e2e for the 64-bit paths: i64/u64 roundtrips with
 * bigint-exact values (including everything beyond
 * `Number.MAX_SAFE_INTEGER`) and the bool parameter/return path,
 * against the release cdylib through bun:ffi.
 */

import { describe, expect, test } from "bun:test";

import { ErrorCode, hasArtifact, native, takeError } from "./load.ts";

const skip = !(await hasArtifact());

const I64_MIN = -(2n ** 63n);
const I64_MAX = 2n ** 63n - 1n;
const U64_MAX = 2n ** 64n - 1n;

describe.skipIf(skip)("P1 type matrix (bigint + bool paths)", () => {
  test("mirror_i64 roundtrips -2^63 bigint-exactly", () => {
    expect(native.mirrorI64(I64_MIN)).toBe(I64_MIN);
  });

  test("mirror_i64 roundtrips -1, 0 and 1", () => {
    expect(native.mirrorI64(-1n)).toBe(-1n);
    expect(native.mirrorI64(0n)).toBe(0n);
    expect(native.mirrorI64(1n)).toBe(1n);
  });

  test("mirror_i64 roundtrips 2^63-1 bigint-exactly", () => {
    expect(native.mirrorI64(I64_MAX)).toBe(I64_MAX);
  });

  test("mirror_u64 roundtrips 0 and 2^53+1", () => {
    expect(native.mirrorU64(0n)).toBe(0n);
    // 2^53 + 1 is the first integer a JS `number` cannot represent:
    // only the bigint path survives it exactly.
    expect(native.mirrorU64(2n ** 53n + 1n)).toBe(2n ** 53n + 1n);
  });

  test("mirror_u64 roundtrips 2^64-1 bigint-exactly", () => {
    expect(native.mirrorU64(U64_MAX)).toBe(U64_MAX);
  });

  test("the 64-bit out-parameters stay bigint-exact across magnitudes", () => {
    const unsigned = [
      2n ** 31n,
      2n ** 32n + 1n,
      2n ** 40n + 15n,
      2n ** 53n,
      I64_MAX - 1n,
      U64_MAX - 1n,
    ];
    for (const sample of unsigned) {
      expect(native.mirrorU64(sample)).toBe(sample);
    }
    const signed = [-(2n ** 31n), -(2n ** 40n + 15n), -(2n ** 53n), I64_MIN + 1n];
    for (const sample of signed) {
      expect(native.mirrorI64(sample)).toBe(sample);
    }
  });

  test("is_even reports parity through the bool path", () => {
    expect(native.isEven(0n)).toBe(true);
    expect(native.isEven(2n)).toBe(true);
    expect(native.isEven(2n ** 63n)).toBe(true);
    expect(native.isEven(U64_MAX - 1n)).toBe(true);
    expect(native.isEven(1n)).toBe(false);
    expect(native.isEven(I64_MAX)).toBe(false);
    expect(native.isEven(U64_MAX)).toBe(false);
  });

  test("a null out-pointer on the bigint path becomes NullPointer", () => {
    // The generated shim must reject the null __ret slot instead of
    // crashing the host (same guard the add test exercises).
    const { status, error } = native.mirrorI64NullOut(1n);
    expect(status).toBe(ErrorCode.NullPointer);
    expect(error).toBeInstanceOf(TypeError);
    expect(error?.message).toBe("output pointer is null");
    // The slot was drained by takeError.
    expect(takeError()).toBeNull();
  });
});
