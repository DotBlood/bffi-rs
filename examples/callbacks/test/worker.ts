/**
 * Worker-side half of `callbacks.test.ts` phase C.
 *
 * Bun workers are threads of the SAME process, so the dlopen'ed
 * library, its Rust statics and the process-global JS-thread binding
 * are shared with the main thread.
 *
 * The worker binds ITS thread first (the first binder wins; the
 * binding is sticky for the process lifetime), then enters the
 * BLOCKING drain. Blocking is intentional and safe here: the worker
 * has already reported "bound", `loop_run()` only returns once the
 * main thread calls `loop_stop()` (which the test does before
 * terminating the worker), and worker threads exist precisely to run
 * blocking work off the main thread.
 */
import { findProjectRoot, loadConfigFile, localArtifactPath } from "@z2net/bffi";

import { createApiFromJson } from "../.bffi/api.gen.ts";

const found = await findProjectRoot(import.meta.dir);
if (found === undefined) {
  throw new Error(".bffi/bffi.json not found above the worker");
}
const root = found;
const config = await loadConfigFile(root);
const api = createApiFromJson(localArtifactPath(config, root));

// From now on, direct callback invokes from the main thread are
// rejected with WrongThread (12).
const bindStatus = api.bind_js_thread();
postMessage({ kind: "bound", bindStatus });

// Blocks until the main thread calls loop_stop(); executes every
// marshalled job (each under the release boundary policy).
const executed = api.loop_run();
postMessage({ kind: "loop-done", executed });
