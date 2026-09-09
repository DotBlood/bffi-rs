# examples/callbacks - both directions, the thread gate, the marshal

The callback surface of the framework (`bffi-callback`): the
JS -> Rust lifecycle, the Rust -> JS ownership, the wrong-thread
rejection and the marshal delivery onto a worker thread - through
the GENERIC callback ABI (`bffi_callback_*` exports + the wire
codec), driven by the `@z2net/bffi` helpers.

## File map

```
examples/callbacks/
├── Cargo.toml            # the bffi stack + bffi-callback + bffi-event-loop
├── .bffi/                # bffi.json (features: runtime + callbacks), loader JSON, api.gen.ts
├── src/
│   ├── lib.rs            # the surface + bffi_callback_abi!() expansion
│   ├── module_def.rs     # the single ModuleDef aggregation
│   └── bin/emit_json.rs  # writes .bffi/bffi.api.json
└── test/
    ├── callbacks.test.ts  # 4 phased e2e tests (ORDER-DEPENDENT)
    └── worker.ts          # binds ITS thread, then enters the blocking drain
```

Three ABI macros are expanded in the crate:

- `bffi_build::bffi_runtime_abi!()` - the error drain + buffer pair;
- `bffi_callback::bffi_callback_abi!()` - `bffi_callback_set_thread`,
  `_bind`, `_invoke`, `_revoke`: the four generic exports JavaScript
  composes through `setJsThread` / `bindJsCallback` /
  `invokeCallback` / `revokeCallback` (wire-encoded signatures,
  arguments and results: `[tag][payload]`, `I32 = 1`).

## The two callback tables

| Table | Tag | Created by | Addressed by |
| --- | --- | --- | --- |
| native (JS -> Rust) | `0x0200` | `bffi_callback::register(sig, Arc<body>)` | opaque handle |
| JS-bound (Rust -> JS) | `0x0201` | the generic `bffi_callback_bind` ABI (a raw JSCallback pointer stored under a fresh handle) | opaque handle |

`invoke` is unified for the NATIVE direction: signature check ->
JS-thread gate -> the registered body. A JS-BOUND handle is an
OPAQUE token: Rust never dereferences it (a live call INTO JS is the
async delivery path's job - see examples/async); the example reads
the token back through `callback_ptr` (`js_callback`) to prove
storage, identity and revocation.

## The JS-thread policy

`ensure_js_thread` gates every invocation: while the process is
UNBOUND it admits every caller; after the FIRST `set_js_thread`
somewhere, only that thread passes and everyone else is rejected
with `WrongThread (12)`. The binding is process-global and STICKY -
there is no unbind. This is why the tests are phased.

## The surface

| Export | Kind | Purpose |
| --- | --- | --- |
| `callback_register() -> u64` | typed | registers the native doubling body `i32(i32)`; JS invokes it via `invokeCallback` (generic ABI) |
| `callback_invoke_status(u64, i32) -> u32` | status variant | the same `invoke` returning the RAW `ErrorCode` - the exact codes are pinnable (`0` Ok, `12` WrongThread, `4` InvalidHandle) |
| `callback_ptr(u64) -> u64` | typed | reads the JS-bound pointer token back (`js_callback`) - identity and revocation for the Rust -> JS direction |
| `bind_js_thread() -> u32` | status variant | `set_js_thread` for the WORKER: first binder wins, sticky |
| `marshal_invoke(u64, i32) -> u32` | status variant | queues a job that invokes the callback ON THE RUNNER THREAD and stores the result; `12` when no runner is up |
| `last_invoked() -> i32` | probe | the value stored by the last marshal job |
| `loop_run() -> u64` | blocking | the worker's drain (jobs invoke callbacks, so the worker must own the JS-thread binding first) |
| `loop_stop()` | sticky | unblocks `loop_run` |

## The e2e suite and its PHASES (test/callbacks.test.ts)

One file, strict order - the JS-thread binding is process-global and
irreversible, so everything that needs the process UNBOUND runs
first.

1. **pipeline load** - the full `await bffi()` chain plus a second
   `dlopen` of the same artifact with `buildDeclarations(moduleJson,
   features)`: the typed API hides the generic exports, the raw
   symbol table exposes them to the JS-side helpers;
2. **phase A (JS -> Rust, unbound)** - `callback_register`, then
   `invokeCallback(raw, handle, 21) == 42`; an EMPTY argument slice
   against the registered `i32(i32)` signature throws "signature
   mismatch"; revocation is terminal - a dead handle reports
   "revoked", and the SECOND revoke reports it too;
3. **phase B (Rust -> JS, unbound)** - a JS function is wrapped into
   a bun:ffi `JSCallback`; the generic bind ABI stores the pointer
   (`sig = [ret tag, param tags]`, here `[TAG_I32, TAG_I32]`);
   `callback_ptr` reads the token back and it equals the pointer
   handed over; revocation is terminal for this direction as well;
4. **phase C (worker, wrong thread + marshal)** - the worker calls
   `bind_js_thread` (first binder wins) and enters `loop_run`.
   From the main thread: `callback_invoke_status == 12` and the
   generic invoke throws "non-JS thread"; `marshal_invoke` is
   retried until it returns `0` (the worker posts "bound" BEFORE
   entering the drain); the job executes ON THE WORKER (where the
   gate passes) and stores `42`; `loop_stop` unblocks the runner
   (executed exactly once - the failed marshal retries never
   enqueued); finally a LATE `setJsThread` from the main thread is
   rejected - the stickiness proof.

## Why phases and not parallel tests

Everything sticky lives in ONE dlopen'ed library shared by the whole
process: the JS-thread binding, the loop state, the tables. A second
test file would race the binding under bun's parallel test runner.
Within the file, bun runs tests sequentially in declaration order -
the phases lean on that.

## Run

```sh
bun test examples/callbacks   # from the REPOSITORY ROOT
```
