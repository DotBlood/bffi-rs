# examples/event-loop - the queue, the drains, the marshal

The event-loop surface of the framework (`bffi-event-loop`) stripped
to its mechanics: enqueue a job, drain it exactly once, marshal to a
RUNNING loop, block a worker thread in the draining `run()`, and shut
the loop down stickily. The jobs are deliberately trivial (`x * 2`
into a process-wide slot) - what the example verifies is the DELIVERY
machinery itself.

## File map

```
examples/event-loop/
├── Cargo.toml            # the bffi stack + bffi-event-loop (features: runtime only)
├── .bffi/                # bffi.json, loader JSON, api.gen.ts
├── src/
│   ├── lib.rs            # the eight exports over bffi-event-loop
│   ├── module_def.rs     # the single ModuleDef aggregation
│   └── bin/emit_json.rs  # writes .bffi/bffi.api.json
└── test/
    ├── event-loop.test.ts # 6 e2e tests (ORDER-DEPENDENT)
    └── worker.ts          # the worker half of the run()/stop() phase
```

## What the event loop IS

Native code cannot hook Bun's real event loop. `bffi-event-loop` is
the honest next thing: a thread-safe job queue (`Mutex<VecDeque>` +
`Condvar`, deliberately - a job queue sees a handful of transitions
per job and does not need the lock-free machinery of the handle
tables) plus two drains:

- **`pump()`** - non-blocking: pops and executes every queued job,
  stops at an empty queue, never waits. Safe from any thread,
  concurrently with `run()`: the queue mutex serializes pops and only
  the popper runs the job it popped, so every job executes EXACTLY
  ONCE.
- **`run()`** - blocking: the calling thread becomes the primary
  runner and waits on the condvar until `stop()`; additional runners
  are drain-only (they exit on an empty queue). Every job executes
  under `run_extern_body`: a panicking job becomes a stored last
  error and the loop lives on.

A **job** is `Box<dyn FnOnce() + Send + 'static>`.

## The surface

| Export | Signature | Behaviour |
| --- | --- | --- |
| `enqueue_job(x: i64)` | `Result<(), LoopError>` | queues a job storing `x * 2`; fails once the loop is stopped |
| `last_result() -> i64` | probe | the value stored by the last executed job |
| `loop_pump() -> u64` | probe | jobs executed by THIS pump call |
| `loop_pending() -> u64` | probe | jobs waiting in the queue |
| `loop_executed() -> u64` | probe | jobs executed since process start, across all runners |
| `marshal_status(x: i64) -> u32` | status variant | `marshal` = enqueue REQUIRING a runner; returns the raw `ErrorCode`: `12` (`WrongThread`) without one, `0` with |
| `loop_run() -> u64` | blocking | `run()` for a worker thread; returns the jobs executed by THIS runner |
| `loop_stop()` | sticky | wakes every runner; `enqueue` afterwards reports "the event loop has been stopped" |

## Why `marshal_status` returns the raw code

A typed `#[bffi]` `Result` flattens every failure to
`ErrorCode::DomainError (13)` - the WrongThread code would be masked
behind the domain channel. The status variant (u32 return + last
error) keeps the EXACT code observable, which is what the tests pin:
`12` without a runner, `0` with one.

## The e2e suite and its ORDER (test/event-loop.test.ts)

The tests are order-dependent ON PURPOSE; keep the order.

1. **pipeline load** - the full `await bffi()` chain (cold-build
   timeout: 10 minutes);
2. **the empty queue drains nothing** - `pump() == 0`, `pending == 0`;
3. **enqueue then pump, exactly once** - `pending == 1`; one pump
   executes the job (`last_result == 42`, `executed` advanced); a
   second pump finds the queue empty;
4. **marshal without a runner = WrongThread (12)** - no runner has
   EVER started in this process at this point, so the status is
   deterministic and the result slot stays untouched;
5. **worker phase** - a `Worker` (a thread of the SAME process, so
   the dlopen'ed library and the queue are shared) posts "ready" and
   enters the blocking `loop_run()`. The main thread retries
   `marshal_status` until it returns `0` (the worker posts BEFORE
   entering the drain - a marshal between those two moments sees no
   runner yet), waits for `last_result == 42` (the job executed ON
   THE WORKER), then calls `loop_stop()`: the sticky stop wakes the
   runner, the worker reports its executed count, `terminate()` is
   belt and suspenders;
6. **after the stop** - `enqueue_job` throws "the event loop has been
   stopped" (stop is sticky for the process lifetime).

## Why blocking `run()` on a worker is safe

Worker threads exist precisely to run blocking work off the main
thread. The worker has already reported "ready"; `run()` returns
only once the main thread calls `loop_stop()` - which the test does
before terminating the worker. Nothing is polled, nothing spins: the
runner sleeps on the condvar between jobs.

## Run

```sh
bun test examples/event-loop  # from the REPOSITORY ROOT
```
