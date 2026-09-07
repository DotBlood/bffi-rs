/**
 * End-to-end tests: the release cdylib loaded through bun:ffi - the
 * first real crossing of the bffi C ABI (closes the P1-SPEC §10.5
 * risk). The artifact must exist: run `bun run build` first
 * (`bun run test:native` does both). Debug builds are never loaded -
 * the boom test relies on the release panic policy.
 */

import { describe, expect, test } from "bun:test";

import { Counter, ErrorCode, hasArtifact, native, takeError } from "./load.ts";

const skip = !(await hasArtifact());

describe.skipIf(skip)("native artifact (release cdylib)", () => {
  test("add writes the result through the out-parameter", () => {
    expect(native.add(2, 3)).toBe(5);
    expect(native.add(0, 0)).toBe(0);
  });

  test("greet travels as a NUL-terminated cstring", () => {
    native.greet("Bun");
    expect(native.greetLen()).toBe(3);
    native.greet("привет");
    // 2 cyrillic letters * 2 bytes + 4 single-byte letters
    expect(native.greetLen()).toBe(12);
  });

  test("boom becomes ErrorCode::Panic with the panic message", () => {
    const { status, error } = native.boom();
    expect(status).toBe(ErrorCode.Panic);
    expect(error?.message).toBe("boom");
    // A caught panic stores no source: no cause reaches JS.
    expect(error?.cause).toBeUndefined();
  });

  test("the last error is drained after a take", () => {
    expect(takeError()).toBeNull();
  });

  test("a null out-parameter becomes NullPointer and a JS TypeError", () => {
    const { status, error } = native.addNullOut(1, 2);
    expect(status).toBe(9); // ErrorCode::NullPointer
    expect(error).toBeInstanceOf(TypeError);
    expect(error?.message).toBe("output pointer is null");
    // The slot was drained by takeError.
    expect(takeError()).toBeNull();
  });

  test("shout returns a JS string through the buffer pair", () => {
    expect(native.shout("Bun")).toBe("HELLO Bun!");
  });

  test("checked_div transports Ok through the out-parameter", () => {
    expect(native.checkedDiv(10, 2)).toBe(5);
  });

  test("checked_div reports Err as a domain JS Error (code 13)", () => {
    try {
      native.checkedDiv(1, 0);
      throw new Error("expected checkedDiv to throw");
    } catch (error) {
      expect(error).toBeInstanceOf(Error);
      expect((error as Error).message).toBe("division by 0");
      // The source is the MathError whose Display equals the message:
      // the honest contract is presence AND equality.
      expect((error as Error).cause).toBe("division by 0");
      expect((error as Error).cause).toBe((error as Error).message);
    }
  });

  test("the Counter class roundtrips through its shims", () => {
    const counter = new Counter(41);
    expect(counter.value).toBe(41);
    // &self methods compute from the stored value; the stored value
    // itself only changes through a new instance (no interior
    // mutability in this example).
    expect(counter.increment()).toBe(42);
    expect(counter.value).toBe(41);
    counter.release();
    // After release the handle is dead: the getter reports
    // InvalidHandle.
    expect(() => counter.value).toThrow();
  });

  test("a second Counter instance owns an independent handle", () => {
    const a = new Counter(1);
    const b = new Counter(100);
    expect(a.increment()).toBe(2);
    expect(b.increment()).toBe(101);
    a.release();
    b.release();
  });

  test("echo_buffer roundtrips bytes through the buffer pair", () => {
    const payload = Uint8Array.from([1, 2, 3, 250, 251]);
    const { status, error, handle } = native.echoBuffer(payload);
    expect(status).toBe(ErrorCode.Ok);
    expect(error).toBeNull();
    expect(handle).not.toBe(0n);
    expect(native.freeBuffer(handle)).toBe(ErrorCode.Ok);
    expect(native.freeBuffer(handle)).toBe(ErrorCode.InvalidHandle);
  });

  test("echo_buffer with an empty buffer still yields a handle", () => {
    const { status, handle } = native.echoBuffer(new Uint8Array(0));
    expect(status).toBe(ErrorCode.Ok);
    expect(handle).not.toBe(0n);
    expect(native.freeBuffer(handle)).toBe(ErrorCode.Ok);
  });
});
