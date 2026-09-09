# @z2net/bffi-native

The [bffi-rs](https://github.com/z2net/bffi-rs) reference native
module, distributed as **pattern A** (napi-rs style): this main
package is pure TypeScript; every platform ships as its own npm
package carrying the prebuilt cdylib. npm/Bun installs only the one
matching the running platform through `optionalDependencies`.

| Package | Platform | Binary |
| --- | --- | --- |
| `@z2net/bffi-native-win32-x64-msvc` | Windows x64 | `bffi_native.dll` |
| `@z2net/bffi-native-linux-x64-gnu` | Linux x64 (glibc) | `libbffi_native.so` |
| `@z2net/bffi-native-darwin-aarch64` | macOS arm64 | `libbffi_native.dylib` |

**Bun only** (>= 1.4.0).

## Usage

```ts
import { createNative } from "@z2net/bffi-native";

const native = createNative(); // resolves + dlopens the platform binary

native.add(3, 4);        // => 7
native.shout("bffi");    // => "HELLO bffi!"
native.version();        // => "0.1.0"
```

An explicit library path overrides platform resolution (tests,
locally built artifacts):

```ts
const native = createNative("D:/path/to/bffi_native.dll");
```

## The surface

The Rust side (`crates/bffi-native` in the repository) is
deliberately minimal - a REAL bffi surface crossing the FFI
boundary, exercising the loader's type matrix:

- `add(a: u32, b: u32) -> u32` - primitives and the out-parameter;
- `shout(name: string) -> string` - cstring parameter, string return
  through the transient-buffer pair;
- `version() -> string` - static string return.

## Rebuilding

```sh
cargo build --release -p bffi-native
bunx @z2net/bffi-cli pack --src target/release/bffi_native.dll \
  --triple win32-x64-msvc --name @z2net/bffi-native
```

## License

MIT - see [LICENSE](./LICENSE).
