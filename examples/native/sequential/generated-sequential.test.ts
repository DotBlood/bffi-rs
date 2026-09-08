/**
 * SEQUENTIAL end-to-end tests for the generated loader (P4): tests
 * that must not race other files under the parallel `bun test`
 * runner.
 *
 * - async delivery pumps the PROCESS-WIDE event loop: the legacy
 *   `callbacks.test.ts` issues a sticky `loopStop()`, so a pump-
 *   driven async test in a parallel file depends on scheduling luck;
 * - the generic callback exports bind the JS thread process-wide
 *   (sticky), so they need a predictable thread.
 *
 * This file runs in its own process, after the main suite
 * (`bun run test:native`); requires the release artifact.
 */

import { describe, expect, test } from "bun:test";
import { dlopen } from "bun:ffi";

import { createApiFromJson, moduleJson } from "../js/api.gen.ts";
import {
  bindJsCallback,
  buildDeclarations,
  invokeCallback,
  revokeCallback,
  setJsThread,
} from "../../../packages/bffi-loader/src/index.ts";
import { sym } from "../../../packages/bffi-loader/src/error.ts";
import type { FfiLib } from "../../../packages/bffi-loader/src/index.ts";
import { artifactPath, hasArtifact } from "../js/env.ts";

const skip = !(await hasArtifact());

// This file MUST NOT run inside a parallel `bun test` sweep: the
// sticky JS-thread bind and the pump-driven async delivery race other
// test files (seen on CI: callbacks.test.ts bindStatus 12). It runs
// on its own, gated by the env variable:
//
//   BFFI_SEQUENTIAL_E2E=1 bun test generated-sequential
const gated = process.env.BFFI_SEQUENTIAL_E2E !== "1";

const artifact = artifactPath();

const api = createApiFromJson(artifact);

/** Declarations of the HAND-WRITTEN verification exports this suite
 * drives (they have no descriptors, so `buildDeclarations` does not
 * know them). */
const HARNESS_DECLARATIONS = {
  // pump-driven async delivery (the event loop)
  example_loop_pump: { args: ["pointer"], returns: "u32" },
  // registers the doubling body (sig i32(i32)) -> handle
  example_callback_register: { args: ["pointer"], returns: "u32" },
  // reads back a bound JS-callback pointer token (identity check)
  example_js_callback_get: { args: ["u64", "pointer"], returns: "u32" },
} as const;

function rawLib(): FfiLib {
  const { symbols } = dlopen(artifact, {
    ...buildDeclarations(moduleJson),
    ...HARNESS_DECLARATIONS,
  });
  return symbols as unknown as FfiLib;
}

/** Jobs executed by this pump call (the non-blocking drain). */
function pump(lib: FfiLib): bigint {
  const out = new BigUint64Array(1);
  const status = sym(lib, "example_loop_pump")(out);
  if (status !== 0) {
    throw new Error(`example_loop_pump failed: ${String(status)}`);
  }
  return out[0] ?? 0n;
}

/** Registers the doubling callback body; returns its handle. */
function registerDoubling(lib: FfiLib): { handle: bigint; status: number } {
  const out = new BigUint64Array(1);
  const status = sym(lib, "example_callback_register")(out);
  return { handle: out[0] ?? 0n, status: Number(status) };
}

/** Reads back the bound JS-callback token (identity check). */
function jsCallbackToken(lib: FfiLib, handle: bigint): { status: number } {
  const out = new BigUint64Array(1);
  const status = sym(lib, "example_js_callback_get")(handle, out);
  return { status: Number(status) };
}

describe.skipIf(skip || gated)("generated loader, sequential e2e", () => {
  test("async exports resolve through the pumped loop (exampleCompute)", async () => {
    const promise = api.example_compute(21);
    const deadline = Date.now() + 5000;
    let settled = false;
    void promise.then(
      () => {
        settled = true;
      },
      () => {
        settled = true;
      },
    );
    while (!settled) {
      if (Date.now() > deadline) {
        throw new Error("timed out waiting for example_compute");
      }
      pump(rawLib());
      await new Promise((wake) => setTimeout(wake, 1));
    }
    // NOTE: the P3 async descriptor says `Promise<number>`, but the
    // u32 payload travels as AsyncValue::I64 - the runtime value is a
    // bigint (same behavior as the legacy loader). Q7 in the spec.
    await expect(promise as Promise<unknown>).resolves.toBe(42n);
  });

  test("the thread binds once for this process", () => {
    const lib = rawLib();
    // Idempotent on the JS thread; WrongThread only when ANOTHER
    // thread won the bind (never in this sequential process).
    setJsThread(lib);
  });

  test("JS invokes the registered native body (invoke/revoke)", () => {
    const lib = rawLib();
    // The doubling body is registered by the hand-written
    // `example_callback_register` export (sig i32(i32)).
    const registration = registerDoubling(lib);
    expect(registration.status).toBe(0);
    expect(invokeCallback(lib, registration.handle, 21)).toBe(42);
    revokeCallback(lib, registration.handle);
    expect(() => invokeCallback(lib, registration.handle, 1)).toThrow();
  });

  test("signature mismatches surface as InvalidArgument", () => {
    const lib = rawLib();
    const registration = registerDoubling(lib);
    try {
      // Empty args vs the declared i32(i32): a deliberate mismatch.
      expect(() => invokeCallback(lib, registration.handle)).toThrow(
        /callback signature mismatch/,
      );
    } finally {
      revokeCallback(lib, registration.handle);
    }
  });

  test("JS functions bind for the Rust side (bind/revoke)", () => {
    const lib = rawLib();
    const bound = bindJsCallback(
      lib,
      { ret: "i32", params: ["i32"] },
      (a: unknown) => (typeof a === "number" ? a + 1 : 0),
    );
    expect(bound.handle).toBeGreaterThan(0n);
    // Rust can read the stored token back through the hand-written
    // identity export.
    const info = jsCallbackToken(lib, bound.handle);
    expect(info.status).toBe(0);
    bound.revoke();
  });
});
