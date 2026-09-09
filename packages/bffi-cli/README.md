# @z2net/bffi-cli

Bun-only CLI for [bffi-rs](https://github.com/DotBlood/bffi-rs):
scaffold, build, validate, generate and publish native modules
through the [`@z2net/bffi`](https://www.npmjs.com/package/@z2net/bffi)
pipeline. A thin wrapper - all pipeline logic lives in the library.

**Bun only** (>= 1.4.0).

## Install

```sh
bun add -d @z2net/bffi-cli
# or run without installing:
bunx @z2net/bffi-cli --help
```

Installs the `bffi` executable. Run it with `bunx bffi <command>` (or
`bun run bffi` from package scripts).

## Commands

| Command | Purpose |
| --- | --- |
| `bffi init [--root <dir>] [--module <m>] [--crate-name <n>] [--binary <b>]` | scaffold `.bffi/bffi.json` + a minimal Rust cdylib crate (with the `emit-json` bin) |
| `bffi build [--config <p>] [--root <d>]` | `cargo build --release -p <crate>` for the configured crate |
| `bffi check [--config <p>] [--root <d>]` | validate the project WITHOUT building: config, loader JSON, generated file, artifact |
| `bffi doctor [--config <p>] [--root <d>]` | `check` PLUS the environment: bun runtime version, cargo executable |
| `bffi codegen [--config <p>] [--root <d>]` | loader JSON -> the typed `api.gen.ts` module |
| `bffi pack --src <binary> --triple <t> [--name <base>] [--out <dir>]` | assemble one platform npm package (pattern A) from a built cdylib: binary + `os`/`cpu`/`libc` package.json + `{ path }` shim |
| `bffi fetch --repo <owner/repo> --tag <v> --asset <file> [--out <dir>]` | download a prebuilt binary (+ `.sha256` sidecar) from a GitHub Release and verify the digest |

`--help` on the bare command prints the full usage.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | success |
| 1 | usage error (missing/unknown arguments) |
| 2 | input or environment failure (bad config, missing files, old bun, no cargo) |

## A typical loop

```sh
bunx bffi init --module mylib     # .bffi/bffi.json + crate skeleton
# ...implement the Rust surface...
bunx bffi build                   # cargo build --release
bunx bffi check                   # config/json/generated/artifact - green
bunx bffi pack --src target/release/bffi_mylib.dll --triple win32-x64-msvc --name @scope/mylib
```

The pack step is one half of pattern-A distribution (napi-rs style):
the main package pins every `@scope/mylib-<triple>` platform package
in `optionalDependencies`; consumers resolve the binary with
`resolvePlatformBinary` from `@z2net/bffi`.

## License

MIT - see [LICENSE](./LICENSE).
