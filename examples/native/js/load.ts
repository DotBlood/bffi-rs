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
  Panic: 2,
  InvalidHandle: 4,
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
  // bffi_runtime_abi!() exports
  bffi_error_take_last: { args: [], returns: "u64" },
  bffi_error_name: { args: ["u64"], returns: "u32" },
  bffi_error_message_ptr: { args: ["u64"], returns: "ptr" },
  bffi_error_message_len: { args: ["u64"], returns: "u64" },
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
 * Drains the thread-local last error into a JS `Error`.
 * Returns `null` when no error is stored.
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
  symbols.bffi_error_free(handle);
  const Ctor = JS_ERROR_NAMES[name] ?? Error;
  return new Ctor(message);
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
  boom(): { status: number; error: Error | null } {
    // boom's result never materializes (it panics), but the out-slot
    // must still be passed per the ABI contract.
    const out = new Uint32Array(1);
    return callWithStatus(lib().bffi_boom(ptr(out)));
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
