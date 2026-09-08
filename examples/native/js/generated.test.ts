/**
 * End-to-end parity tests for the GENERATED loader (P4): the same
 * native behaviors `load.ts` covers, driven through the codegen'ed
 * `api.gen.ts` module. Every path of the type matrix gets one test;
 * hand-written `bffi_extern!` exports (callback doubling body, loop
 * probes) stay reachable through the raw symbol table built by
 * `buildDeclarations` - the loader appends the generic callback
 * exports automatically.
 *
 * Requires the release artifact (`bun run test:native`).
 */

import { describe, expect, test } from "bun:test";

import { createApiFromJson } from "./api.gen.ts";
import { hasArtifact } from "./env.ts";

const skip = !(await hasArtifact());

const artifact = `${import.meta.dir}/../../../target/release/${
  process.platform === "win32" ? "" : "lib"
}bffi_example_native.${process.platform === "win32" ? "dll" : process.platform === "darwin" ? "dylib" : "so"}`;

const api = createApiFromJson(artifact);

// The generic callback exports have their own SEQUENTIAL e2e file
// (generated-callbacks.e2e.ts): the sticky JS-thread binding races
// other test files under the parallel `bun test` runner.

describe.skipIf(skip)("generated loader parity", () => {
  test("number params and the u32 out slot (add)", () => {
    expect(api.add(3, 4)).toBe(7);
  });

  test("cstring parameter and the unit return (greet/greetLen)", () => {
    api.greet("generated");
    expect(api.greet_len()).toBe(9);
  });

  test("buffer-handle string returns decode (shout)", () => {
    expect(api.shout("gen")).toBe("HELLO gen!");
  });

  test("ptr_len parameters: empty buffers cross as a null pointer", () => {
    expect(api.echo_buffer(new Uint8Array([9, 8, 7]))).toEqual(new Uint8Array([9, 8, 7]));
    expect(api.echo_buffer(new Uint8Array(0))).toEqual(new Uint8Array(0));
  });

  test("Result errors surface as thrown Errors with the cause (checkedDiv)", () => {
    expect(api.checked_div(6, 3)).toBe(2);
    try {
      api.checked_div(1, 0);
      throw new Error("expected checked_div to throw");
    } catch (error) {
      expect((error as Error).message).toBe("division by 0");
    }
  });

  test("release panics surface as thrown Errors (boom)", () => {
    expect(() => api.boom()).toThrow(/boom/);
  });

  test("bigint paths keep exactness (mirrorI64/mirrorU64)", () => {
    expect(api.mirror_i64(-5n)).toBe(-5n);
    const big = 9_223_372_036_854_775_808n;
    expect(api.mirror_u64(big)).toBe(big);
  });

  test("bool params and returns coerce across u8 (is_even)", () => {
    expect(api.is_even(4n)).toBeTrue();
    expect(api.is_even(5n)).toBeFalse();
  });

  test("the facade-mode probe works unchanged (facade_probe)", () => {
    expect(api.facade_probe(3)).toBe(9);
  });

  test("classes wrap the handle; getters, methods and release work", () => {
    const counter = new api.counter(10);
    expect(counter.value).toBe(10);
    // The Rust body is `&self` - increment returns the next value
    // without mutating the stored field.
    expect(counter.increment()).toBe(11);
    expect(counter.value).toBe(10);
    counter.release();
  });

  // The async-parity test lives in generated-sequential.e2e.ts: the
  // legacy `callbacks.test.ts` issues a sticky `loopStop()` and the
  // parallel `bun test` runner races files, so a pump-driven async
  // test in THIS file would depend on scheduling luck.
});
