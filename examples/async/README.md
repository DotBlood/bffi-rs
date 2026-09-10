# examples/async - Rust futures as JS Promises

The async surface of the framework (`bffi-async`): JavaScript awaits
Rust futures as native `Promise`s, with domain errors, panics,
timeouts and cooperative cancellation - all crossing the bffi
boundary through the full `@z2net/bffi` pipeline. The e2e suite IS
the documentation: every delivery path the runtime supports has one
test.

## File map

```
examples/async/
├── Cargo.toml            # the bffi stack (features: async)
├── .bffi/                # bffi.json (features: runtime + async), loader JSON, api.gen.ts
├── src/
│   ├── lib.rs            # #[bffi_async] fns + the manual spawn/cancel exports
│   ├── module_def.rs     # the single ModuleDef aggregation
│   └── bin/emit_json.rs  # writes .bffi/bffi.api.json
└── test/async.test.ts    # 9 e2e tests
```

Three ABI macros are expanded in the crate (they must live in the
USER cdylib - the linker drops `#[no_mangle]` symbols from rlibs):

- `bffi_build::bffi_runtime_abi!()` - the error drain + buffer pair;
- `bffi_async::bffi_async_abi!()` - `bffi_async_attach` /
  `bffi_async_cancel`, the two exports the JS side drives tasks with.

## The threading model (the Bun-support invariant)

Three roles, never mixed:

1. **the JS thread** - the only place legal to invoke resolver
   callbacks; it also drains the event loop (`loop_pump`);
2. **executor workers** - N = machine parallelism capped at 4; they
   poll the spawned futures and never touch JavaScript;
3. **the timer thread** - deadlines for `sleep` / `timeout`.

A completed future is delivered by ENQUEUEING a job onto the event
loop. Consequence (DESIGN §7): **promises settle only while the JS
side drains the loop** - nothing magical wakes Bun's tick.

## What `#[bffi_async]` generates

For `#[bffi_async] pub async fn double_async(x: u64) -> u64` the
macro emits:

- a spawn shim `bffi_double_async(x: u64, __ret: *mut u64) ->
  ErrorCode`: converts parameters, spawns the future on the executor,
  writes the TASK HANDLE to `__ret`;
- a descriptor `bffi_meta_double_async::FUNCTION` whose return type
  is `Promise<u64>` - the loader's `decodeReturn` sees the task ABI
  and hands the handle to `wrapTask`, so the typed JS call returns a
  `Promise<bigint>` directly.

Owned parameters cross by copy inside the shim (before the spawn):
`String` arrives as a cstring pointer, `Vec<u8>` as a `(ptr, len)`
pair; both are copied into owned data that outlives the call.
Borrowed `&str` / `&[u8]` are REJECTED at compile time (E002) -
borrowings cannot survive a spawn.

## The delivery path (why pumping is the contract)

```
worker: future completes
  -> outcome encoded into the wire format [tag][payload]
     (i64 without f64 narrowing, Str with a u32 length prefix, ...)
  -> stored into a transient buffer (the runtime buffer table)
  -> bffi_event_loop::enqueue(job)          (thread-safe queue)
JS thread (during pump):
  -> the job runs: reads the buffer handle, calls the resolve (or
     reject) JSCallback trampoline  (unsafe, JS thread only)
  -> the Promise settles; JS decodes the payload and frees the buffer
```

That is why the test pumps: `pumpUntil(promise, () => api.loop_pump())`
(from `@z2net/bffi`) awaits the promise while calling the crate's
`loop_pump` export - a one-liner over `bffi_event_loop::pump()` - in
a loop. The loader NEVER starts a hidden interval; the pump is an
explicit contract.

## The surface

| Export | Kind | Proves |
| --- | --- | --- |
| `double_async(u64) -> u64` | `#[bffi_async]` | value delivery: `Promise<bigint>`, `10n` for `5n` after a 15 ms sleep |
| `shout_async(String) -> String` | `#[bffi_async]` | string payload through the wire record (`HELLO async!`) |
| `fail_async() -> Result<u64, ExampleError>` | `#[bffi_async]` | domain rejection with the exact message |
| `panic_async() -> u64` | `#[bffi_async]` | a worker panic caught by `catch_unwind` becomes the rejection `"async boom"` (the backtrace in stderr is expected noise) |
| `timed_async() -> Result<u64, ExampleError>` | `#[bffi_async]` | the `timeout` combinator drops the inner future at the 50 ms deadline -> `"task timed out"` |
| `spawn_slow(ms) -> u64` | manual `bffi_async::spawn` | the RAW task handle as a plain return - JS keeps it un-wrapped |
| `cancel_task(u64) -> u32` | manual `bffi_async::cancel` | cooperative cancellation: the future is dropped at the next poll boundary; the attached promise rejects `"task cancelled"` |
| `async_pending() -> u64` | probe | spawned-but-unfinished task count (0 after every terminal state) |
| `loop_pump() -> u64` | probe | the non-blocking drain that drives all deliveries |

## Why the raw-handle path exists alongside the macro

The typed API wraps every task handle into a Promise IMMEDIATELY
(`decodeReturn` -> `wrapTask`), so JS never sees the handle - there is
nothing to cancel with. `spawn_slow` keeps the handle in JS-land:
the test wraps it itself (`wrapTask(raw, task)`), asserts a SECOND
attach on the same task is rejected ("already attached"), cancels,
and observes the rejection. This is the low-level half of the same
machinery the macro automates.

## The e2e suite (test/async.test.ts)

1. the full pipeline builds, generates and loads (cold-build
   timeout: 10 minutes);
2. value delivery while pumping;
3. string payload decoding;
4. domain rejection;
5. panic rejection;
6. timeout rejection (inner future actually dropped);
7. "delivered by pumping, not by magic": without a pump the settled
   flag stays false after 60 ms; one `pumpUntil` later it resolves;
8. cancellation through the raw handle + the double-attach guard;
9. `async_pending() == 0` - every task reached a terminal state.

## Run

```sh
bun test examples/async      # from the REPOSITORY ROOT
```
