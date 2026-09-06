/**
 * Callback-surface e2e: JS -> Rust lifecycle, Rust -> JS ownership and
 * the wrong-thread + marshal contract, exercised against the release
 * cdylib through bun:ffi (kanboard criteria 5.1, 5.2, 5.3).
 *
 * SPIKE FINDINGS (Bun 1.4.0, win64 - recorded per the task contract):
 *
 * 1. `JSCallback` (bun:ffi) turns a JS function into a native pointer
 *    passable to an FFI export: `new JSCallback(fn, { args: ["i32"],
 *    returns: "i32" })`. The raw pointer is the `.ptr` property and is
 *    a plain NUMBER (observed 2044152786432) - it can be passed
 *    directly wherever a "u64" argument is declared; compare exactly
 *    via `BigInt(cb.ptr)` against the handle-sized roundtrip.
 *    `.close()` releases the trampoline; the prototype owns exactly
 *    ["constructor", "close"]. The JSCallback object must stay
 *    referenced for the pointer to stay meaningful.
 * 2. `import type { ... } from "./api"` is erased at runtime: a file
 *    whose ONLY use of a specifier is `import type` executes even when
 *    that specifier does not exist on disk (module resolution never
 *    runs), so `api-usage.ts` can exist for tsc alone and must never
 *    be imported by any test.
 *
 * ONE phased test on purpose: the JS-thread binding (phase C) is
 * process-global and sticky, so phases must run in strict order inside
 * a single `test()` - earlier phases rely on the process still being
 * unbound, and nothing can run after the worker binds except the
 * marshal path itself.
 */

import { JSCallback } from "bun:ffi";
import { describe, expect, test } from "bun:test";

import { ErrorCode, hasArtifact, native } from "./load.ts";

const skip = !(await hasArtifact());

/** Waits for one named message from the phase-C worker. */
function waitFor(
  worker: Worker,
  kind: string,
  timeoutMs = 10_000,
): Promise<{ kind: string; bindStatus?: number; runStatus?: number }> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`timeout waiting for the ${kind} message`));
    }, timeoutMs);
    worker.addEventListener("message", (event: MessageEvent) => {
      const data = event.data as { kind?: string };
      if (data?.kind === kind) {
        clearTimeout(timer);
        resolve(event.data as { kind: string; bindStatus?: number; runStatus?: number });
      }
    });
    worker.addEventListener("error", (event: ErrorEvent) => {
      clearTimeout(timer);
      reject(event.error ?? new Error("worker error"));
    });
  });
}

describe.skipIf(skip)("callback surface (criteria 5.1-5.3)", () => {
  test(
    "phased lifecycle: JS->Rust, Rust->JS ownership, wrong-thread + marshal",
    async () => {
      // -----------------------------------------------------------
      // Phase A - JS -> Rust lifecycle, process still UNBOUND
      // (ensure_js_thread admits every caller while unbound).
      // -----------------------------------------------------------

      // Criterion 5.1: register -> invoke(21) == 42 through the
      // signature-checked doubling closure.
      const registered = native.callbackRegister();
      expect(registered.status).toBe(ErrorCode.Ok);
      expect(registered.error).toBeNull();
      const handle = registered.handle;
      expect(handle).not.toBe(0n);

      const doubled = native.callbackInvoke(handle, 21);
      expect(doubled.status).toBe(ErrorCode.Ok);
      expect(doubled.value).toBe(42);
      expect(doubled.error).toBeNull();

      // Criterion 5.2 (arity half): an EMPTY argument slice against the
      // registered i32(i32) signature surfaces SignatureMismatch as
      // InvalidArgument (11) plus the deterministic expected/got
      // message.
      const mismatched = native.callbackInvokeMismatched(handle);
      expect(mismatched.status).toBe(ErrorCode.InvalidArgument);
      expect(mismatched.error?.message).toContain("signature mismatch");
      expect(mismatched.error?.message).toContain("expected i32(i32)");
      expect(mismatched.error?.message).toContain("got (none)");

      // Criterion 5.2 (terminal revocation): revoke, then the dead
      // handle reports InvalidHandle (4) on invoke...
      expect(native.callbackRevoke(handle).status).toBe(ErrorCode.Ok);
      const dead = native.callbackInvoke(handle, 21);
      expect(dead.status).toBe(ErrorCode.InvalidHandle);
      expect(dead.error?.message).toContain("revoked");

      // ...and the SECOND revoke reports InvalidHandle too: the Rust
      // wrapper maps the crate's `false` (already revoked, idempotent
      // no-op) to InvalidHandle plus a last error, so the terminal
      // contract is observable across the ABI.
      const again = native.callbackRevoke(handle);
      expect(again.status).toBe(ErrorCode.InvalidHandle);
      expect(again.error?.message).toContain("revoked");

      // -----------------------------------------------------------
      // Phase B - Rust -> JS ownership (criterion 5.1). The JSCallback
      // pointer roundtrips through the Rust-side slot verbatim.
      // -----------------------------------------------------------
      const jsCallback = new JSCallback((x: number) => x * 2, {
        args: ["i32"],
        returns: "i32",
      });
      expect(jsCallback.ptr).not.toBeNull();
      const fnPointer = jsCallback.ptr ?? -1;
      expect(fnPointer).toBeGreaterThan(0);

      const bound = native.jsCallbackBind(fnPointer);
      expect(bound.status).toBe(ErrorCode.Ok);
      const jsHandle = bound.handle;
      expect(jsHandle).not.toBe(0n);

      // The read-back token equals the pointer we handed over.
      const got = native.jsCallbackGet(jsHandle);
      expect(got.status).toBe(ErrorCode.Ok);
      expect(got.pointer).toBe(BigInt(fnPointer));

      // Criterion 5.2 for the JS direction: revoke -> get is
      // InvalidHandle (4).
      expect(native.jsCallbackRevoke(jsHandle).status).toBe(ErrorCode.Ok);
      const revoked = native.jsCallbackGet(jsHandle);
      expect(revoked.status).toBe(ErrorCode.InvalidHandle);
      expect(revoked.error?.message).toContain("revoked");
      jsCallback.close();

      // -----------------------------------------------------------
      // Phase C - Worker + wrong-thread + marshal (criterion 5.3).
      // The worker binds ITS thread (first binder wins, sticky for the
      // process) and enters the blocking run() drain (pump() is the
      // documented mock - see js/worker.ts for the choice).
      // -----------------------------------------------------------
      const phaseC = native.callbackRegister();
      expect(phaseC.status).toBe(ErrorCode.Ok);
      const workerHandle = phaseC.handle;

      const worker = new Worker(new URL("./worker.ts", import.meta.url));
      const boundMessage = await waitFor(worker, "bound");
      expect(boundMessage.bindStatus).toBe(ErrorCode.Ok);

      // The main thread is now the WRONG thread: the direct invoke is
      // rejected with WrongThread (12) - the P1 half of the criterion.
      const rejected = native.callbackInvoke(workerHandle, 1);
      expect(rejected.status).toBe(ErrorCode.WrongThread);
      expect(rejected.error?.message).toContain("non-JS thread");

      // The P2 half: the job is MARSHALED to the worker's run() loop.
      // The runner may not be up yet when "bound" arrives (the worker
      // posts before entering run()), so retry until the marshal is
      // accepted; marshal without a runner reports WrongThread (12).
      const deadline = Date.now() + 10_000;
      let marshalStatus = native.loopMarshalInvoke(workerHandle, 21);
      while (marshalStatus !== ErrorCode.Ok && Date.now() < deadline) {
        await Bun.sleep(5);
        marshalStatus = native.loopMarshalInvoke(workerHandle, 21);
      }
      expect(marshalStatus).toBe(ErrorCode.Ok);

      // The job runs ON THE WORKER THREAD (where invoke passes the JS
      // -thread gate) and stores 42 into the process-wide slot.
      let invoked = native.lastInvoked();
      while (invoked !== 42 && Date.now() < deadline) {
        await Bun.sleep(5);
        invoked = native.lastInvoked();
      }
      expect(invoked).toBe(42);

      // Loop probes: the queue is drained, the global executed counter
      // advanced, and pump() is the documented always-0 mock.
      expect(native.loopPending()).toBe(0n);
      expect(native.loopExecuted() >= 1n).toBe(true);
      expect(native.loopPump()).toBe(0n);

      // Shutdown: the sticky stop unblocks the worker's run(), the
      // worker reports its executed count (exactly the one marshalled
      // job - retries that saw "no runner" never enqueued), then
      // terminate() is belt and suspenders.
      expect(native.loopStop()).toBe(ErrorCode.Ok);
      const done = await waitFor(worker, "loop-done");
      expect(done.runStatus).toBe(1);
      worker.terminate();
    },
    20_000,
  );
});
