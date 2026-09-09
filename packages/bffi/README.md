# @z2net/bffi

Bun-only runtime loader for [bffi-rs](https://github.com/DotBlood/bffi-rs)
native modules: the typed JS API is built from the loader schema the
Rust side emits (`bffi_build::loader_json`, schema version 1), so no
per-function binding is ever written by hand. Includes
`resolvePlatformBinary` for pattern-A distribution (platform packages
carrying prebuilt binaries).

## Layout

- `src/wire.ts` - the `[tag][payload]` value codec shared with the
  Rust side (`bffi_types::wire`): async task payloads and callback
  arguments use the same table.
- `src/error.ts` - ErrorCode table and the `takeError` drain over the
  runtime ABI (`bffi_error_*`).
- `src/buffer.ts` - transient-buffer reading (`bffi_buffer`,
  `bffi_buffer_length`, `bffi_types_free`); bytes are copied BEFORE
  the free.
- `src/loader.ts` - schema types, `assertSchema`, and
  `buildDeclarations` turning the ABI view into `bun:ffi` dlopen
  declarations (`bool` crosses as `"u8"`, `ptr_len` expands into the
  `("ptr", "u64")` pair).
- `src/async.ts` - `wrapTask` (task handle to `Promise` through the
  resolve/reject JSCallbacks) and `pumpUntil` (explicit event-loop
  pumping; the loader never starts a hidden interval).
- `src/classes.ts` / `src/api.ts` - the typed factory:
  `createApi(schema, libraryPath)` opens the library and builds the
  object; `ApiOf<Schema>` derives the TypeScript types from the
  schema literal itself, so the generated module ships exact
  signatures with zero hand-written types.
- `src/callbacks.ts` - callback surface types (implementation lands
  together with the generic callback ABI exports).

## Rules carried over from the reference loader

- an empty `Uint8Array` parameter crosses as a null data pointer with
  `len == 0` (bun:ffi rejects empty TypedArrays as pointers);
- buffer reads copy the bytes before `bffi_types_free`;
- JSCallback `returns` is `"void"` (bun:ffi does not know
  `"undefined"`);
- an empty buffer payload is indistinguishable from `Option::None` -
  nullable returns surface `null` for both (same limitation as the
  reference loader).

## Tests

```sh
bun test packages/bffi
```

The suite covers the codec round-trips against the Rust-side vectors,
declaration building, argument encoding, and the API factory over a
mock symbol table. The real-dlopen end-to-end pass lives in
`examples/native/js` (stage T6).
