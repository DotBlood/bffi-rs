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
- [crates/bffi-build/CALLING-CONVENTION.md](https://github.com/z2net/bffi-rs/blob/main/crates/bffi-build/CALLING-CONVENTION.md) - the C ABI contract (every crossing, callback exports included)
- [docs/CONTRIBUTING.md](https://github.com/z2net/bffi-rs/blob/main/docs/CONTRIBUTING.md) - how to contribute (branching, commits, PRs)
- [AGENTS.md](https://github.com/z2net/bffi-rs/blob/main/AGENTS.md) - engineering rules for humans and AI agents
- [packages/bffi-loader](https://github.com/z2net/bffi-rs/blob/main/packages/bffi-loader) - the JS runtime loader (see its README)
- [SECURITY.md](https://github.com/z2net/bffi-rs/blob/main/SECURITY.md) - security policy
- [CONTACT.md](https://github.com/z2net/bffi-rs/blob/main/CONTACT.md) - contacts

## Requirements

- [Bun](https://bun.sh) >= 1.4.0 (enforced at runtime by `bffi-loader` and the `bffi` CLI)
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
| `packages/bffi` | Bun-only JS integration package: config, full pipeline (build → json → api.gen), typed loader (`@z2net/bffi`, publication pending) |
| `packages/bffi-cli` | The `bffi` CLI: init, build, codegen, pack, fetch, check, doctor (`@z2net/bffi-cli`, publication pending) |
| `packages/native-template` | COPY-ME template: pattern-A npm packaging of a native module (reference-only) |

## Getting started

```sh
bun install          # installs dependencies + git hooks (lefthook)
bun run build        # builds the example native module (release cdylib)
bun run test:native  # builds it and runs the bun:ffi e2e suite
bun run check        # oxlint + tsc + cargo check
bun run ci           # full CI parity: lint, typecheck, fmt, clippy, tests
```

For a native module, depend on [`bffi`](https://github.com/z2net/bffi-rs/blob/main/crates/bffi)
(the facade: one dependency for the whole stack) and - when using the
attribute macros - on the individual `bffi-core`/`bffi-types`/
`bffi-dts` crates their expansions name.

## Generated TypeScript API

The `#[bffi]` descriptors are the single source of truth: the same
aggregated `ModuleDef` renders the committed `.d.ts`, the canonical
loader JSON and the typed TS module - byte-deterministic, safe to
commit and diff.

```sh
# build time (in your crate):
cargo run --bin emit-json                          # writes js/bffi.api.json
bun bffi codegen js/bffi.api.json -o js/api.gen.ts # typed TS module
```

```ts
import { createApiFromJson } from "./api.gen.ts";

const api = createApiFromJson("./target/release/libmy.so");
api.add(1, 2);                       // number, typed; errors throw JS Errors
const counter = new api.counter(10); // classes: FinalizationRegistry + release()
await api.compute(21);               // `#[bffi_async]` -> Promise
```

A full worked example lives in
[`examples/native`](https://github.com/z2net/bffi-rs/blob/main/examples/native)
(shims, classes, async and callbacks through real `bun:ffi`, with a
46-test e2e parity suite).

## Conventions

- Conventional Commits are enforced by a `commit-msg` hook (`scripts/commit-msg.sh`).
- Pre-commit runs oxlint, `tsc --noEmit`, `cargo fmt --check` and clippy.
- Pre-push runs the workspace tests.
- GitHub Actions CI is planned; until it lands, `bun run ci` is the source of truth.

## License

[MIT](https://github.com/z2net/bffi-rs/blob/main/LICENSE)
