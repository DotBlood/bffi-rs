# bffi-rs

<div align="center">

[![Bun](https://img.shields.io/badge/Bun-%3E%3D1.4.0-F472B6?logo=bun&logoColor=white)](https://bun.sh)
[![Rust](https://img.shields.io/badge/Rust-1.98.0-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-3DA639?logo=opensourceinitiative&logoColor=white)](https://github.com/z2net/bffi-rs/blob/main/LICENSE)
[![GitHub Issues](https://img.shields.io/github/issues/z2net/bffi-rs)](https://github.com/z2net/bffi-rs/issues)
[![GitHub Pull Requests](https://img.shields.io/github/issues-pr/z2net/bffi-rs)](https://github.com/z2net/bffi-rs/pulls)

**English** | [Русский](https://github.com/z2net/bffi-rs/blob/main/docs/i18n/ru/README.md) | [简体中文](https://github.com/z2net/bffi-rs/blob/main/docs/i18n/zh-CN/README.md)

</div>

Binding framework for Bun - a napi-rs-equivalent for [Bun](https://bun.sh), built on `bun:ffi` and a thin C ABI. Written in Rust, bottom-up from small focused crates.

See [docs/DESIGN.md](https://github.com/z2net/bffi-rs/blob/main/docs/DESIGN.md) for architecture and [AGENTS.md](https://github.com/z2net/bffi-rs/blob/main/AGENTS.md) for the project's engineering rules.

## Documentation

- [docs/DESIGN.md](https://github.com/z2net/bffi-rs/blob/main/docs/DESIGN.md) - architecture & decisions
- [crates/bffi/CALLING-CONVENTION.md](https://github.com/z2net/bffi-rs/blob/main/crates/bffi/CALLING-CONVENTION.md) - the C ABI contract (every crossing, callback exports included)
- [docs/CONTRIBUTING.md](https://github.com/z2net/bffi-rs/blob/main/docs/CONTRIBUTING.md) - how to contribute (branching, commits, PRs)
- [AGENTS.md](https://github.com/z2net/bffi-rs/blob/main/AGENTS.md) - engineering rules for humans and AI agents
- [packages/bffi](https://github.com/z2net/bffi-rs/blob/main/packages/bffi) - `@z2net/bffi`: the typed loader + build pipeline (see its README)
- [packages/bffi-cli](https://github.com/z2net/bffi-rs/blob/main/packages/bffi-cli) - `@z2net/bffi-cli`: the `bffi` CLI (init, build, check, doctor, codegen, pack, fetch)
- [packages/native](https://github.com/z2net/bffi-rs/blob/main/packages/native) - `@z2net/bffi-native`: the reference native module (platform npm package family)
- [examples/sqlite](https://github.com/z2net/bffi-rs/blob/main/examples/sqlite) - entry example (the full pipeline over rusqlite); `examples/async`, `examples/event-loop`, `examples/callbacks` sit beside it, each doubling as an e2e suite
- [SECURITY.md](https://github.com/z2net/bffi-rs/blob/main/SECURITY.md) - security policy
- [CONTACT.md](https://github.com/z2net/bffi-rs/blob/main/CONTACT.md) - contacts

## Requirements

- [Bun](https://bun.sh) >= 1.4.0 (enforced at runtime by `@z2net/bffi` and the `bffi` CLI)
- Rust 1.98.0 (pinned via `rust-toolchain.toml`; rustup installs it automatically)
- bash (for the commit-msg hook; preinstalled on macOS/Linux, Git Bash on Windows)

## Components

| Part | Purpose |
| ---- | ------- |
| `crates/bffi-core` | Generational handles, lock-free tables, catch_unwind boundary |
| `crates/bffi-types` | Number/string/buffer conversion, SIMD UTF-8, the shared wire codec |
| `crates/bffi-error` | Unified `BffiError` -> JS Error mapping |
| `crates/bffi-object` | `ObjectWrap<T>` ownership over the global `Registry` |
| `crates/bffi-callback` | Callbacks in both directions + the generic callback ABI |
| `crates/bffi-dts` | TypeScript IR + deterministic `.d.ts` renderer |
| `crates/bffi-macros` | `#[bffi]` / `#[bffi_async]` (shims + descriptors) |
| `crates/bffi-class` | `#[bffi_class]` / `#[bffi_impl]` over `ObjectWrap` |
| `crates/bffi-macro-support` | Shared macro internals (kinds, classification, codegen) |
| `crates/bffi-event-loop` | `run()` / `pump()` job queue on the JS thread |
| `crates/bffi-build` | Runtime ABI exports, transient buffers, `.d.ts`/loader-JSON emitters |
| `crates/bffi-async` | `#[bffi_async]`: Rust futures as JS Promises (cancel, timeout, tokio opt-in) |
| `crates/bffi` | The facade: one dependency re-exporting the whole stack |
| `crates/bffi-native` | The reference cdylib (`add`/`shout`/`version` + runtime ABI); source of the `@z2net/bffi-native` platform package family |
| `packages/bffi` | Bun-only JS integration package: config, full pipeline (build → json → api.gen), typed loader (npm: `@z2net/bffi`) |
| `packages/bffi-cli` | The `bffi` CLI: init, build, codegen, pack, fetch, check, doctor (npm: `@z2net/bffi-cli`) |

## Getting started

```sh
bun install          # installs dependencies + git hooks (lefthook)
bun run build        # builds all four example crates (release cdylibs)
bun run test:e2e     # runs the examples as e2e suites (bun test examples)
bun run check        # oxlint + tsc + cargo check
bun run ci           # full CI parity: lint, typecheck, fmt, clippy, tests
```

For a native module, depend on [`bffi`](https://github.com/z2net/bffi-rs/blob/main/crates/bffi)
(the facade: one dependency for the whole stack) and - when using the
attribute macros - on the individual `bffi-core`/`bffi-types`/
`bffi-dts` crates their expansions name.

## Generated TypeScript API

The `#[bffi]` descriptors are the single source of truth: the crate's
`emit-json` binary writes `.bffi/bffi.api.json` (schema v1) from the
aggregated `ModuleDef`, and the `@z2net/bffi` pipeline does the rest -
validate, generate `.bffi/api.gen.ts`, resolve and `dlopen` the
library. Deterministic bytes, safe to commit and diff.

```ts
import { bffi } from "@z2net/bffi";
import type { Api } from "./.bffi/api.gen.ts";

const api: Api = await bffi();       // one call: build -> json -> gen -> dlopen
api.add(1, 2);                       // number, typed; errors throw JS Errors
const counter = new api.counter(10); // classes: FinalizationRegistry + release()
await api.compute(21);               // `#[bffi_async]` -> Promise
```

The pipeline, its config (`.bffi/bffi.json`) and every subtlety are
documented in
[`packages/bffi`](https://github.com/z2net/bffi-rs/blob/main/packages/bffi);
a full worked example lives in
[`examples/sqlite`](https://github.com/z2net/bffi-rs/blob/main/examples/sqlite).
Async, event-loop and callbacks each have a dedicated example
(`examples/async`, `examples/event-loop`, `examples/callbacks`), and
every example doubles as an e2e suite (`bun test examples`).

## Conventions

- Conventional Commits are enforced by a `commit-msg` hook (`scripts/commit-msg.sh`).
- Pre-commit runs oxlint, `tsc --noEmit`, `cargo fmt --check` and clippy.
- Pre-push runs the workspace tests.
- GitHub Actions CI (`.github/workflows/ci.yml`) runs on every pull request and on pushes to `main` / `dev/main` (Rust matrix: ubuntu / windows / macos, plus a JS job); `bun run ci` remains the local parity command.

## License

[MIT](https://github.com/z2net/bffi-rs/blob/main/LICENSE)
