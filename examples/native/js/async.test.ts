/**
 * End-to-end tests for bffi-async through real bun:ffi: Rust futures
 * spawned from JS as Promises, cancellation and timeouts (the P3
 * criteria), with resolutions delivered while the JS thread pumps the
 * event loop.
 */

import { describe, expect, test } from "bun:test";

import { ErrorCode, hasArtifact, native, nativeAsync, wrapTask } from "./load.ts";

const skip = !(await hasArtifact());

/** Wraps a task handle into a Promise (see load.ts wrapTask). */
function wrap(task: bigint): Promise<unknown> {
  return wrapTask(task);
}

/** Awaits `promise` while pumping the event loop - resolutions are
 * delivered on the JS thread during the drain. */
function withPump<T>(promise: Promise<T>, timeoutMs = 5000): Promise<T> {
  const iv = setInterval(() => native.loopPump(), 2);
  const deadline = new Promise<never>((_, rejectTimeout) => {
    setTimeout(() => rejectTimeout(new Error("timed out waiting for the task")), timeoutMs);
  });
  return Promise.race([promise, deadline]).finally(() => clearInterval(iv));
}

describe.skipIf(skip)("bffi-async through real bun:ffi", () => {
  test("await resolves a Rust future value through the buffer pair", async () => {
    const task = nativeAsync.double(5);
    await expect(withPump(wrap(task))).resolves.toBe(10n);
  });

  test("string results decode from the transient-buffer payload", async () => {
    const task = nativeAsync.shout("async");
    await expect(withPump(wrap(task))).resolves.toBe("HELLO async!");
  });

  test("a failing task rejects with the domain message", async () => {
    const task = nativeAsync.fail();
    await expect(withPump(wrap(task))).rejects.toThrow("domain failure");
  });

  test("a panicking task rejects with the panic message", async () => {
    const task = nativeAsync.panic();
    await expect(withPump(wrap(task))).rejects.toThrow("async boom");
  });

  test("a timeout cancels the inner future and rejects", async () => {
    const task = nativeAsync.timeout();
    await expect(withPump(wrap(task))).rejects.toThrow("task timed out");
  });

  test("cancellation from JS rejects with 'task cancelled'", async () => {
    const task = nativeAsync.timeout(); // never completes on its own
    // Cancel before the 50 ms deadline expires.
    const promise = wrap(task);
    const status = nativeAsync.cancel(task);
    expect(status).toBe(ErrorCode.Ok);
    await expect(withPump(promise)).rejects.toThrow("task cancelled");
  });

  test("a second attach is rejected with InvalidArgument", async () => {
    const task = nativeAsync.fail();
    const first = withPump(wrap(task));
    await expect(first).rejects.toThrow("domain failure");
    // The result is already delivered: attaching again surfaces the
    // sticky attach error.
    expect(() => wrap(task)).toThrow(/already attached/);
  });

  test("the bffi_async macro drives a full async roundtrip", async () => {
    const task = nativeAsync.compute(5);
    await expect(withPump(wrap(task))).resolves.toBe(10n);
  });

  test("resolutions are delivered by pumping, not by magic", async () => {
    const task = nativeAsync.double(21);
    const settled: { value?: unknown; error?: Error } = {};
    const promise = wrap(task).then(
      (value) => {
        settled.value = value;
        return value;
      },
      (error: Error) => {
        settled.error = error;
        throw error;
      },
    );
    // No withPump: the pumpUntil loop below IS the delivery driver.
    await pumpUntil(() => "value" in settled || "error" in settled);
    await expect(promise).resolves.toBe(42n);
    expect(settled.value).toBe(42n);
  });
});

/** Pumps the event loop until `settled` or the deadline expires. */
async function pumpUntil(settled: () => boolean, timeoutMs = 5000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (!settled()) {
    if (Date.now() > deadline) {
      throw new Error("timed out waiting for the promise to settle");
    }
    native.loopPump();
    await new Promise((wake) => setTimeout(wake, 1));
  }
}

