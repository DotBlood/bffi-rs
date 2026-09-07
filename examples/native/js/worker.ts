/**
 * Worker-side half of `callbacks.test.ts` phase C (criterion 5.3).
 *
 * Bun workers are threads of the SAME process, so the dlopen'ed
 * library, its Rust statics and the process-global JS-thread binding
 * are shared with the main thread.
 *
 * Pump-vs-run choice: `pump()` is a real non-blocking drain now, but
 * the worker still enters the BLOCKING drain
 * (`example_loop_run`, i.e. `bffi_event_loop::run`). Blocking is
 * intentional and safe here: the worker has already reported
 * "bound", `run()` only returns once the main thread calls
 * `example_loop_stop()` (which the test does before terminating the
 * worker), and worker threads exist precisely to run blocking work
 * off the main thread.
 */
import { native } from "./load.ts";

// Binds THIS thread as the process-wide JS thread: the first binder
// wins and the binding is sticky for the process lifetime. From now
// on, direct invokes from the main thread are rejected with
// WrongThread (12) - the P1 half of criterion 5.3.
const bindStatus = native.setJsThread();
postMessage({ kind: "bound", bindStatus });

// Blocks until the main thread calls example_loop_stop(); executes
// every marshalled job (each under the release boundary policy).
const runStatus = native.loopRun();
postMessage({ kind: "loop-done", runStatus });
