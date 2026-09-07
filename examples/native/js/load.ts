/**
 * Loader for the example native module: dlopen declarations and thin
 * helpers over the bffi C ABI (CALLING-CONVENTION.md is the contract).
 *
 * This file is the reference sketch for the facade's future JS loader.
 * Bun-only: paths are resolved from `import.meta.dir`, the artifact is
 * probed with `Bun.file().exists()`, and the library is dlopen'ed
 * lazily so `bun test` can skip cleanly when the release artifact has
 * not been built yet.
 */

import { dlopen, ptr, toArrayBuffer } from "bun:ffi";

/** ErrorCode values that cross the C ABI (bffi-core/src/error.rs). */
export const ErrorCode = {
  Ok: 0,
  Error: 1,
  Panic: 2,
  InvalidHandle: 4,
  NullPointer: 9,
  InvalidArgument: 11,
  WrongThread: 12,
  DomainError: 13,
} as const;

/** `bffi_error_name` values (CALLING-CONVENTION.md §5). */
const JS_ERROR_NAMES: Record<number, ErrorConstructor> = {
  1: Error,
  2: TypeError,
  3: RangeError,
};

const LIB_EXT =
  process.platform === "win32"
    ? "dll"
    : process.platform === "darwin"
      ? "dylib"
      : "so";

// js/ -> native/ -> examples/ -> repository root. Forward slashes are
// used everywhere: Bun's fs/ffi accept them on every platform.
const ROOT = (() => {
  const parts = import.meta.dir.replaceAll("\\", "/").split("/");
  parts.splice(-3);
  return parts.join("/");
})();

/** Absolute path of the release cdylib artifact. */
export function artifactPath(): string {
  return `${ROOT}/target/release/bffi_example_native.${LIB_EXT}`;
}

/** Whether the release artifact has been built (`bun run build`). */
export async function hasArtifact(): Promise<boolean> {
  return await Bun.file(artifactPath()).exists();
}

const DECLARATIONS = {
  // #[bffi] shims: every non-unit return travels through the trailing
  // `__ret` out-parameter; the C ABI return is ALWAYS the ErrorCode.
  bffi_add: { args: ["u32", "u32", "pointer"], returns: "u32" },
  bffi_greet: { args: ["cstring"], returns: "u32" },
  bffi_greet_len: { args: ["pointer"], returns: "u32" },
  bffi_boom: { args: ["pointer"], returns: "u32" },
  bffi_shout: { args: ["cstring", "pointer"], returns: "u32" },
  bffi_checked_div: { args: ["u32", "u32", "pointer"], returns: "u32" },
  // #[bffi] verification exports (P1 type matrix)
  bffi_mirror_i64: { args: ["i64", "pointer"], returns: "u32" },
  bffi_mirror_u64: { args: ["u64", "pointer"], returns: "u32" },
  bffi_is_even: { args: ["u64", "pointer"], returns: "u32" },
  // callback lifecycle (criterion 5.1/5.2)
  example_callback_register: { args: ["pointer"], returns: "u32" },
  example_callback_invoke: { args: ["u64", "i32", "pointer"], returns: "u32" },
  example_callback_invoke_mismatched: { args: ["u64"], returns: "u32" },
  example_callback_revoke: { args: ["u64"], returns: "u32" },
  // Rust -> JS callback ownership (criterion 5.1)
  example_js_callback_bind: { args: ["u64", "pointer"], returns: "u32" },
  example_js_callback_get: { args: ["u64", "pointer"], returns: "u32" },
  example_js_callback_revoke: { args: ["u64"], returns: "u32" },
  // JS-thread binding (criterion 5.3)
  example_set_js_thread: { args: [], returns: "u32" },
  // event-loop probes and marshal delivery (criterion 5.3)
  example_loop_run: { args: ["pointer"], returns: "u32" },
  example_loop_stop: { args: [], returns: "u32" },
  example_loop_pending: { args: ["pointer"], returns: "u32" },
  example_loop_executed: { args: ["pointer"], returns: "u32" },
  example_loop_pump: { args: ["pointer"], returns: "u32" },
  example_loop_marshal_invoke: { args: ["u64", "i32"], returns: "u32" },
  example_last_invoked: { args: ["pointer"], returns: "u32" },
  // #[bffi_class] Counter
  bffi_counter_new: { args: ["u32", "pointer"], returns: "u32" },
  bffi_counter_value_get: { args: ["u64", "pointer"], returns: "u32" },
  bffi_counter_increment: { args: ["u64", "pointer"], returns: "u32" },
  bffi_counter_release: { args: ["u64"], returns: "u32" },
  // bffi_runtime_abi!() exports
  bffi_error_take_last: { args: [], returns: "u64" },
  bffi_error_name: { args: ["u64"], returns: "u32" },
  bffi_error_message_ptr: { args: ["u64"], returns: "ptr" },
  bffi_error_message_len: { args: ["u64"], returns: "u64" },
  bffi_error_cause_ptr: { args: ["u64"], returns: "ptr" },
  bffi_error_cause_len: { args: ["u64"], returns: "u64" },
  bffi_error_free: { args: ["u64"], returns: "u32" },
  bffi_buffer: { args: ["u64"], returns: "ptr" },
  bffi_buffer_length: { args: ["u64"], returns: "u64" },
  bffi_types_free: { args: ["u64"], returns: "u32" },
  // hand-written example export (ptr, len parameter sketch)
  example_echo_buffer: { args: ["ptr", "u64", "pointer"], returns: "u32" },
} as const;

function loadSymbols() {
  return dlopen(artifactPath(), DECLARATIONS).symbols;
}

let cached: ReturnType<typeof loadSymbols> | null = null;

/** The dlopen'ed symbols; first call performs the actual dlopen. */
function lib() {
  cached ??= loadSymbols();
  return cached;
}

const decoder = new TextDecoder();

/**
 * Drains the thread-local last error into a JS `Error`. A stored
 * source travels as the standard `Error.cause`. Returns `null` when
 * no error is stored.
 */
export function takeError(): Error | null {
  const symbols = lib();
  const handle = symbols.bffi_error_take_last();
  if (handle === 0n) {
    return null;
  }
  const name = symbols.bffi_error_name(handle);
  const len = Number(symbols.bffi_error_message_len(handle));
  const messagePtr = symbols.bffi_error_message_ptr(handle);
  let message = "";
  if (messagePtr !== null && len > 0) {
    // SAFETY (JS side): the pointer is valid until bffi_error_free and
    // covers exactly `len` UTF-8 bytes (CALLING-CONVENTION.md §5).
    message = decoder.decode(toArrayBuffer(messagePtr, 0, len));
  }
  const causeLen = Number(symbols.bffi_error_cause_len(handle));
  const causePtr = symbols.bffi_error_cause_ptr(handle);
  let cause: string | undefined;
  if (causePtr !== null && causeLen > 0) {
    // SAFETY (JS side): same lifetime contract as the message pair
    // above (CALLING-CONVENTION.md §5); null/0 means no cause.
    cause = decoder.decode(toArrayBuffer(causePtr, 0, causeLen));
  }
  symbols.bffi_error_free(handle);
  const Ctor = JS_ERROR_NAMES[name] ?? Error;
  return cause === undefined ? new Ctor(message) : new Ctor(message, { cause });
}

/** Reads and releases a transient buffer behind a handle. */
export function readBuffer(handle: bigint): Uint8Array {
  const symbols = lib();
  const len = Number(symbols.bffi_buffer_length(handle));
  // The loose .symbols typing unions every return shape; a "ptr"
  // return is a number when non-null, null when NULL.
  const raw = symbols.bffi_buffer(handle);
  const dataPtr = typeof raw === "number" ? raw : null;
  if (dataPtr === null || len === 0) {
    symbols.bffi_types_free(handle);
    return new Uint8Array(0);
  }
  // SAFETY (JS side): same lifetime contract as takeError; the bytes
  // are copied out before the free.
  const bytes = new Uint8Array(toArrayBuffer(dataPtr, 0, len));
  const copy = new Uint8Array(bytes);
  symbols.bffi_types_free(handle);
  return copy;
}

/** Wraps a shim status: returns `[errorCode, takenError]`. */
export function callWithStatus(status: number): { status: number; error: Error | null } {
  return { status, error: status === ErrorCode.Ok ? null : takeError() };
}

export const native = {
  add(a: number, b: number): number {
    const out = new Uint32Array(1);
    const status = lib().bffi_add(a, b, ptr(out));
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`bffi_add failed: ${status}`);
    }
    return out[0] ?? 0;
  },
  /** Deliberately passes a null out-pointer: exercises the shim's
   * NullPointer guard end-to-end. */
  addNullOut(a: number, b: number): { status: number; error: Error | null } {
    return callWithStatus(lib().bffi_add(a, b, null));
  },
  greet(name: string): void {
    const status = lib().bffi_greet(name);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`bffi_greet failed: ${status}`);
    }
  },
  greetLen(): number {
    const out = new Uint32Array(1);
    const status = lib().bffi_greet_len(ptr(out));
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`bffi_greet_len failed: ${status}`);
    }
    return out[0] ?? 0;
  },
  /** Returns the shout as a JS string through the buffer pair. */
  shout(name: string): string {
    const out = new BigUint64Array(1);
    const status = lib().bffi_shout(name, out);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`bffi_shout failed: ${status}`);
    }
    const bytes = readBuffer(out[0] ?? 0n);
    return new TextDecoder().decode(bytes);
  },
  /** Divides; a domain `Err` surfaces as the thrown JS Error. */
  checkedDiv(a: number, b: number): number {
    const out = new Uint32Array(1);
    const status = lib().bffi_checked_div(a, b, out);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`bffi_checked_div failed: ${status}`);
    }
    return out[0] ?? 0;
  },
  boom(): { status: number; error: Error | null } {
    // boom's result never materializes (it panics), but the out-slot
    // must still be passed per the ABI contract.
    const out = new Uint32Array(1);
    return callWithStatus(lib().bffi_boom(ptr(out)));
  },
  /** Mirrors an i64 through the bigint path (P1 type matrix). */
  mirrorI64(x: bigint): bigint {
    const out = new BigInt64Array(1);
    const status = lib().bffi_mirror_i64(x, out);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`bffi_mirror_i64 failed: ${status}`);
    }
    return out[0] ?? 0n;
  },
  /** Mirrors a u64 through the bigint path (P1 type matrix). */
  mirrorU64(x: bigint): bigint {
    const out = new BigUint64Array(1);
    const status = lib().bffi_mirror_u64(x, out);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`bffi_mirror_u64 failed: ${status}`);
    }
    return out[0] ?? 0n;
  },
  /** Reports the parity of a u64 through the bool path (P1 matrix). */
  isEven(x: bigint): boolean {
    const out = new Uint8Array(1);
    const status = lib().bffi_is_even(x, out);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`bffi_is_even failed: ${status}`);
    }
    return (out[0] ?? 0) !== 0;
  },
  /** Deliberately passes a null out-pointer to `bffi_mirror_i64`:
   * exercises the generated shim's NullPointer guard on the bigint
   * path end-to-end. */
  mirrorI64NullOut(x: bigint): { status: number; error: Error | null } {
    return callWithStatus(lib().bffi_mirror_i64(x, null));
  },
  /** Registers the fixed `i32(i32)` doubling callback (criterion 5.1). */
  callbackRegister(): { handle: bigint; status: number; error: Error | null } {
    const out = new BigUint64Array(1);
    const status = lib().example_callback_register(out);
    return {
      handle: out[0] ?? 0n,
      status,
      error: status === ErrorCode.Ok ? null : takeError(),
    };
  },
  /** Invokes the callback behind `handle` with one i32 (criterion 5.1). */
  callbackInvoke(
    handle: bigint,
    a: number,
  ): { value: number; status: number; error: Error | null } {
    const out = new Int32Array(1);
    const status = lib().example_callback_invoke(handle, a, out);
    return {
      value: out[0] ?? 0,
      status,
      error: status === ErrorCode.Ok ? null : takeError(),
    };
  },
  /** Invokes with an empty arg slice: deliberate arity mismatch (5.2). */
  callbackInvokeMismatched(
    handle: bigint,
  ): { status: number; error: Error | null } {
    return callWithStatus(lib().example_callback_invoke_mismatched(handle));
  },
  /** Revokes the callback behind `handle` (criteria 5.1/5.2). */
  callbackRevoke(handle: bigint): { status: number; error: Error | null } {
    return callWithStatus(lib().example_callback_revoke(handle));
  },
  /** Stores a JSCallback pointer as a Rust -> JS callback (5.1). */
  jsCallbackBind(fnPointer: number | bigint): {
    handle: bigint;
    status: number;
    error: Error | null;
  } {
    const out = new BigUint64Array(1);
    const status = lib().example_js_callback_bind(fnPointer, out);
    return {
      handle: out[0] ?? 0n,
      status,
      error: status === ErrorCode.Ok ? null : takeError(),
    };
  },
  /** Reads back the stored pointer token behind `handle` (5.1). */
  jsCallbackGet(handle: bigint): {
    pointer: bigint;
    status: number;
    error: Error | null;
  } {
    const out = new BigUint64Array(1);
    const status = lib().example_js_callback_get(handle, out);
    return {
      pointer: out[0] ?? 0n,
      status,
      error: status === ErrorCode.Ok ? null : takeError(),
    };
  },
  /** Revokes the JS-side callback behind `handle` (5.1/5.2). */
  jsCallbackRevoke(handle: bigint): { status: number; error: Error | null } {
    return callWithStatus(lib().example_js_callback_revoke(handle));
  },
  /** Binds the calling thread as the process-wide JS thread (5.3). */
  setJsThread(): number {
    return lib().example_set_js_thread();
  },
  /** Blocks draining the job queue until stop (worker-side, 5.3). */
  loopRun(): number {
    const out = new BigUint64Array(1);
    const status = lib().example_loop_run(out);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`example_loop_run failed: ${status}`);
    }
    return Number(out[0] ?? 0n);
  },
  /** Stops the loop for good (sticky; main thread, 5.3). */
  loopStop(): number {
    return lib().example_loop_stop();
  },
  /** Jobs waiting for execution (5.3). */
  loopPending(): bigint {
    const out = new BigUint64Array(1);
    const status = lib().example_loop_pending(out);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`example_loop_pending failed: ${status}`);
    }
    return out[0] ?? 0n;
  },
  /** Jobs executed since process start (5.3). */
  loopExecuted(): bigint {
    const out = new BigUint64Array(1);
    const status = lib().example_loop_executed(out);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`example_loop_executed failed: ${status}`);
    }
    return out[0] ?? 0n;
  },
  /** Non-blocking drain probe: jobs executed by this call (5.3). */
  loopPump(): bigint {
    const out = new BigUint64Array(1);
    const status = lib().example_loop_pump(out);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`example_loop_pump failed: ${status}`);
    }
    return out[0] ?? 0n;
  },
  /** Marshals a job that invokes `handle` with `a` on the runner (5.3). */
  loopMarshalInvoke(handle: bigint, a: number): number {
    return lib().example_loop_marshal_invoke(handle, a);
  },
  /** The value stored by the last executed marshal job (5.3). */
  lastInvoked(): number {
    const out = new Int32Array(1);
    const status = lib().example_last_invoked(out);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`example_last_invoked failed: ${status}`);
    }
    return out[0] ?? 0;
  },
  echoBuffer(bytes: Uint8Array): { handle: bigint; status: number; error: Error | null } {
    const out = new BigUint64Array(1);
    // bun:ffi cannot convert an empty TypedArray to a pointer; the
    // Rust side accepts a null data pointer when len == 0.
    const dataPtr = bytes.length > 0 ? ptr(bytes) : null;
    const status = lib().example_echo_buffer(dataPtr, bytes.length, out);
    return {
      handle: out[0] ?? 0n,
      status,
      error: status === ErrorCode.Ok ? null : takeError(),
    };
  },
  freeBuffer(handle: bigint): number {
    return lib().bffi_types_free(handle);
  },
  takeError,
};

/** JS-side wrapper over the generated `Counter` shims. The
 * FinalizationRegistry releases the native handle when the wrapper is
 * collected (the documented P2 JS contract). */
const counterFinalizers = new FinalizationRegistry((handle: bigint) => {
  lib().bffi_counter_release(handle);
});

export class Counter {
  private handle: bigint;

  constructor(start: number) {
    const out = new BigUint64Array(1);
    const status = lib().bffi_counter_new(start, out);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`bffi_counter_new failed: ${status}`);
    }
    this.handle = out[0] ?? 0n;
    counterFinalizers.register(this, this.handle);
  }

  get value(): number {
    const out = new Uint32Array(1);
    const status = lib().bffi_counter_value_get(this.handle, out);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`bffi_counter_value_get failed: ${status}`);
    }
    return out[0] ?? 0;
  }

  increment(): number {
    const out = new Uint32Array(1);
    const status = lib().bffi_counter_increment(this.handle, out);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`bffi_counter_increment failed: ${status}`);
    }
    return out[0] ?? 0;
  }

  release(): void {
    counterFinalizers.unregister(this);
    const status = lib().bffi_counter_release(this.handle);
    if (status !== ErrorCode.Ok) {
      throw takeError() ?? new Error(`bffi_counter_release failed: ${status}`);
    }
  }
}
