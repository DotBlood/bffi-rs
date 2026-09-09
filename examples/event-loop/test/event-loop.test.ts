/**
 * End-to-end test of the FULL `@z2net/bffi` pipeline on the
 * event-loop example: ONE call - cargo build -> loader JSON -> api.gen
 * generation -> binary resolution -> dlopen - then the delivery
 * machinery: enqueue + pump exactly-once, the monitoring probes, the
 * marshal path (12 = WrongThread without a runner, 0 with one), the
 * blocking run on a worker thread and the sticky stop.
 *
 * Run with `bun test examples/event-loop` from the REPO ROOT (the
 * root tsconfig paths resolve `@z2net/bffi`).
 *
 * The tests are ORDER-DEPENDENT: the worker phase binds a runner and
 * `loop_stop` is sticky for the process, so the marshal probe (which
 * requires NO runner) must run first. Keep the order.
 */
import { describe, expect, test } from "bun:test";

import { bffi } from "@z2net/bffi";
import type { Api } from "../.bffi/api.gen.ts";

let api: Api;

/** Waits for one named message from the worker. */
function waitFor(
  worker: Worker,
  kind: string,
  timeoutMs = 10_000,
): Promise<{ kind: string; executed?: bigint }> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`timeout waiting for the ${kind} message`));
    }, timeoutMs);
    worker.addEventListener("message", (event: MessageEvent) => {
      const data = event.data as { kind?: string; executed?: bigint };
      if (data?.kind === kind) {
        clearTimeout(timer);
        resolve(data as { kind: string; executed?: bigint });
      }
    });
    worker.addEventListener("error", (event: ErrorEvent) => {
      clearTimeout(timer);
      reject(event.error ?? new Error("worker error"));
    });
  });
}

describe("event loop through the full pipeline", () => {
  // A COLD release build (LTO, whole workspace) takes minutes; bun's
  // default per-test timeout is 5s.
  test(
    "the pipeline builds, generates and loads the module",
    async () => {
      api = await bffi({ config: `${import.meta.dir}/../.bffi/bffi.json` });
      expect(api).toBeTypeOf("object");
    },
    600_000,
  );

  test("the empty queue drains nothing", () => {
    expect(api.loop_pending()).toBe(0n);
    expect(api.loop_pump()).toBe(0n);
  });

  test("enqueue then pump executes the job exactly once", () => {
    expect(api.enqueue_job(21n)).toBeUndefined();
    expect(api.loop_pending()).toBe(1n);

    expect(api.loop_pump()).toBe(1n);
    expect(api.last_result()).toBe(42n);
    expect(api.loop_pending()).toBe(0n);
    expect(api.loop_executed() >= 1n).toBeTrue();

    // A second pump finds the queue empty: exactly-once.
    expect(api.loop_pump()).toBe(0n);
  });

  test("marshal without a runner reports WrongThread (12)", () => {
    // No runner has ever started in this process: the job cannot be
    // delivered, the status is the WrongThread code and the result
    // slot stays untouched.
    expect(api.marshal_status(5n)).toBe(12);
    expect(api.last_result()).toBe(42n);
  });

  test(
    "a worker thread runs the blocking drain and marshal delivers",
    async () => {
      const worker = new Worker(new URL("./worker.ts", import.meta.url));
      await waitFor(worker, "ready");

      // The runner may not be inside run() yet when "ready" arrives
      // (the worker posts before entering the drain): retry until the
      // marshal is accepted.
      const deadline = Date.now() + 10_000;
      let status = api.marshal_status(21n);
      while (status !== 0 && Date.now() < deadline) {
        await Bun.sleep(5);
        status = api.marshal_status(21n);
      }
      expect(status).toBe(0);

      // The job runs ON THE WORKER THREAD and stores into the
      // process-wide slot.
      while (api.last_result() !== 42n && Date.now() < deadline) {
        await Bun.sleep(5);
      }
      expect(api.last_result()).toBe(42n);

      // Shutdown: the sticky stop unblocks the worker's run(), the
      // worker reports its executed count, then terminate() is belt
      // and suspenders.
      api.loop_stop();
      const done = await waitFor(worker, "loop-done");
      expect((done.executed ?? 0n) >= 1n).toBeTrue();
      worker.terminate();
    },
    20_000,
  );

  test("after the stop, enqueue reports 'stopped' (sticky)", () => {
    expect(() => api.enqueue_job(1n)).toThrow(/stopped/);
  });
});
