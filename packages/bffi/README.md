# @z2net/bffi

Bun-only typed loader and build pipeline for
[bffi-rs](https://github.com/z2net/bffi-rs) native modules: the
typed JS API is built from the loader schema the Rust side emits
(`bffi_build::loader_json`, schema version 1), so no per-function
binding is ever written by hand.

**Bun only** (>= 1.4.0) - no Node.js or Deno support, no runtime
dependencies.

## Install

```sh
bun add @z2net/bffi
```

## The one-call pipeline

`bffi()` runs the whole chain - `cargo build` -> loader JSON ->
`api.gen` generation -> binary resolution -> `dlopen` - and returns
the typed API:

```ts
import { bffi } from "@z2net/bffi";
import type { Api } from "./.bffi/api.gen.ts";

const api: Api = await bffi(); // annotate with the generated type

const handle = api.open(":memory:");
api.exec(handle, "CREATE TABLE t (id INTEGER)");
```

The project is described by ONE config file, `.bffi/bffi.json`
(`defineConfig` / `bffi init` scaffolds it). Everything else - build,
generation, binary path - is derived from it.

## Subpath exports

| Specifier | Contents |
| --- | --- |
| `@z2net/bffi` | the whole public surface (re-exported 1:1 below) |
| `@z2net/bffi/runtime` | wire codec, error drain, buffer pair, `wrapTask`/`pumpUntil`, callback surface |
| `@z2net/bffi/loader` | schema types, `buildDeclarations`, `createApi`, platform-binary resolution |
| `@z2net/bffi/pipeline` | config v1, cargo build step, the `bffi()` orchestrator |
| `@z2net/bffi/codegen` | the deterministic `api.gen.ts` renderer + schema validation |

## Async, callbacks and the event loop

- `wrapTask(lib, taskHandle)` wraps a native task handle into a
  `Promise` (resolve/reject travel through bun:ffi JSCallbacks).
- `pumpUntil(promise, pump)` awaits a task promise while draining the
  native event loop - promise resolutions are delivered BY the pump;
  the loader never starts a hidden interval.
- `bindJsCallback` / `invokeCallback` / `revokeCallback` /
  `setJsThread` drive the generic callback ABI in both directions
  (wire-encoded signatures and arguments).

## Platform packages (pattern A)

Prebuilt binaries ship as per-platform npm packages (napi-rs style):
`@scope/mylib` + `@scope/mylib-win32-x64-msvc`,
`@scope/mylib-linux-x64-gnu`, `@scope/mylib-darwin-aarch64`, ... The
main package declares every platform package in `optionalDependencies`
(exact pins); npm/Bun installs only the matching one.
`resolvePlatformBinary("@scope/mylib")` turns that into the dlopen
path. `@z2net/bffi-cli` (`bffi pack`) assembles platform packages
from a built cdylib.

## Rules the loader lives by

- buffers are COPIED by default; bytes are read before
  `bffi_types_free`;
- an empty `Uint8Array` argument crosses as a null data pointer with
  `len == 0` (bun:ffi rejects empty TypedArrays as pointers);
- an empty buffer payload is indistinguishable from `Option::None` -
  nullable returns surface `null` for both;
- JSCallback `returns` is `"void"` (bun:ffi does not know
  `"undefined"`);
- strings cross the boundary as UTF-8 (`cstring`).

## License

MIT - see [LICENSE](./LICENSE).
