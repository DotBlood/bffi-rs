# bffi-rs - Design Document

**Status:** Done  
**Date:** 2026-09-02  
**License:** MIT  
**Repository:** https://github.com/DotBlood/bffi-rs  
**Contact:** contact@z2net.com

---

## 1. Summary

`bffi-rs` is a native binding framework for writing Rust modules targeting **Bun only**.

It is the Bun counterpart of `napi-rs`, with these differences:

- targets **only Bun** (no Node.js / Deno);
- does **not** depend on Node-API;
- uses `bun:ffi` and a thin C ABI layer;
- is built **bottom-up** from small crates.

Goal: a convenient, relatively safe, and idiomatic way to write high-performance native extensions for Bun.

---

## 2. Motivation

Today most native modules for Bun are written with `napi-rs` (Node-API). This causes:

1. Dependency on a foreign API and execution model.
2. Best-effort Node-API compatibility in Bun (edge-case bugs).
3. Inability to fully leverage Bun-specific features and optimizations.

We want a layer that is native to the Bun ecosystem and has predictable behavior under Bun.

---

## 3. Goals

- Ergonomic Rust → Bun native modules.
- Safe abstractions over C ABI (as far as practical).
- Support for functions, classes, buffers, callbacks, lifetime management, errors.
- Bottom-up development from small, well-tested crates.
- Stable thin C ABI layer at the end.
- TypeScript `.d.ts` generation from day one.
- Fully open source (MIT).

---

## 4. Non-Goals

- Compatibility with Node.js or Deno.
- 100% API compatibility with `napi-rs`.
- Maximum performance at any cost on day one.
- Hiding dangerous operations (zero-copy etc.).

---

## 5. High-level architecture

```
┌─────────────────────────────────────┐
│         User native modules         │
└──────────────────┬──────────────────┘
                   │
┌──────────────────▼──────────────────┐
│              bffi-rs                │  public API + macros
└──────────────────┬──────────────────┘
                   │
┌──────────────────▼──────────────────┐
│     Small crates                    │
│  bffi-types, bffi-object,           │
│  bffi-callback, bffi-class,         │
│  bffi-dts, bffi-error, ...          │
└──────────────────┬──────────────────┘
                   │
┌──────────────────▼──────────────────┐
│            bffi-core                │  foundation + safety rules
└──────────────────┬──────────────────┘
                   │
┌──────────────────▼──────────────────┐
│         Thin C ABI layer            │  called via bun:ffi
└─────────────────────────────────────┘
```

Development order: foundation → one capability at a time → ergonomic API → stable C ABI.

---

## 6. Safety model

Crossing the C ABI drops almost all Rust safety guarantees. Rules:

### 6.1 FFI boundary
- Every `extern "C"` function must be as thin as possible.
- Immediately wrap the body in `catch_unwind`.
- Panics must never cross the FFI boundary in production builds.

### 6.2 Ownership via handles
We use **Generational Index + type-tag**:

```rust
// u64 = (type_tag << 48) | (generation << 24) | index
type Handle = u64;
```

Inside Rust we keep `Arc<T>` (or equivalent) in a table.  
Outside only the opaque handle is visible.

### 6.3 Buffers and strings
- Default = **copy**.
- Zero-copy is allowed only through `bffi::unsafe_zero_copy`.
- Dangerous APIs must be obvious.

### 6.4 Callbacks
- Explicit registration.
- Must not be callable after destruction.
- Wrong-thread calls are either rejected or safely marshalled.

### 6.5 Panics
- **Dev** - may abort (better debugging).
- **Prod** - always converted to a JS `Error`.

---

## 7. Accepted decisions

| Topic            | Decision                                            |
|------------------|-----------------------------------------------------|
| Macro            | `#[bffi]`                                           |
| Minimum Bun      | 1.4.0                                               |
| Rust / Cargo     | 1.98.0                                              |
| Handles          | Generational Index + type-tag                       |
| Buffers          | Copy by default                                     |
| Zero-copy        | Only via `bffi::unsafe_zero_copy`                   |
| Event loop       | Start with `run()`; `pump()` is a mock for now      |
| TypeScript types | Generate from day one (`bffi-dts`)                  |
| Panic (prod)     | Convert to JS Error                                 |
| Panic (dev)      | May abort                                           |
| Compatibility    | Bun only                                            |
| Distribution     | Source in repo; prebuilt binaries later for npm     |
| License          | MIT                                                 |

---

## 8. Components

| Crate              | Purpose                                              | Priority |
|--------------------|------------------------------------------------------|----------|
| `bffi-core`        | Handles, catch_unwind, core utilities, safety rules  | P0       |
| `bffi-types`       | Type conversion (numbers, strings, buffers, …)       | P0       |
| `bffi-error`       | Unified error → JS Error mapping                     | P0       |
| `bffi-object`      | Object ownership / ObjectWrap                        | P1       |
| `bffi-callback`    | Safe callbacks (both directions)                     | P1       |
| `bffi-dts`         | TypeScript `.d.ts` generation                        | P1       |
| `bffi-macros`      | Procedural macros (`#[bffi]`, attributes)            | P1       |
| `bffi-class`       | Class declaration macros                             | P2       |
| `bffi-event-loop`  | `run()` / `pump()` abstraction                       | P2       |
| `bffi-build`       | Build helpers, C ABI generation, Bun integration     | P2       |
| `bffi-rs`          | Public facade that re-exports the stack              | P2       |
| `bffi-async`       | Promise / async support                              | P3       |

---

## 9. API design principles

1. Explicit is better than magical.
2. Safe by default; dangerous paths are explicit.
3. Small crates, one responsibility each.
4. Document ownership contracts on every FFI boundary.
5. Bun-first - no compromises for other runtimes.

---

## 10. Contact

- GitHub Issues / Discussions  
- Email: **contact@z2net.com**
