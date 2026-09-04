# AGENT.md - Rules for AI agents and contributors

[English](https://github.com/DotBlood/bffi-rs/blob/main/AGENT.md) | [Русский](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/AGENT.md) | [简体中文](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/AGENT.md)

This file defines how humans and AI agents must work on **bffi-rs**.

Repository: https://github.com/DotBlood/bffi-rs  
Contact: contact@z2net.com

---

## 1. Project purpose

`bffi-rs` is a **Bun-only** native binding framework written in Rust.

It is the Bun equivalent of `napi-rs`, but:

- targets **only Bun** (no Node.js / Deno compatibility);
- does **not** depend on Node-API;
- uses `bun:ffi` and a thin C ABI layer;
- is built **bottom-up** from small crates.

Primary goals: safety at the FFI boundary, clear ownership, good DX, and long-term maintainability.

Read `DESIGN.md` before making architectural changes.

---

## 2. Hard rules

1. **Bun only**  
   Do not add Node.js or Deno compatibility layers.

2. **Safety first**
   - Default path = copy data, never zero-copy.
   - Zero-copy is allowed only via `bffi::unsafe_zero_copy`.
   - All `extern "C"` functions must be thin and wrapped with `catch_unwind`.

3. **Handles**  
   Use Generational Index + type-tag (`u64`).  
   Never expose raw Rust references or complex types across the C ABI.

4. **Panics**
   - Dev builds may abort (easier debugging).
   - Prod builds must convert panics into JS `Error`.

5. **Minimum Bun version**  
   `1.4.0`

6. **Rust / Cargo version**  
   Project is pinned to **Cargo / Rust 1.98.0**.  
   Do not bump without an explicit decision and CI update.

7. **No secrets in the repo**  
   Anything under `.grok`, `.claude`, `.codex`, `.opencode`, `.hermes`, `.mcp`, `.env`, keys, tokens, etc. must stay out of git (see `.gitignore`).

8. **License**  
   MIT. Keep SPDX headers where appropriate.

---

## 3. Repository structure

```
bffi-rs/
├── AGENT.md                    # this file
├── README.md
├── LICENSE
├── SECURITY.md
├── CONTACT.md
├── Cargo.toml                  # workspace
├── rust-toolchain.toml         # pinned 1.98.0
├── package.json                # Bun workspace / scripts
├── tsconfig.json
├── .oxlintrc.json              # linter config
├── lefthook.yml                # git hooks (lint, fmt, commit-msg)
├── .gitignore
├── .github/
│   ├── ISSUE_TEMPLATE/
│   ├── PULL_REQUEST_TEMPLATE.md
│   └── workflows/              # CI (planned; deferred until bffi-core lands)
├── crates/
│   ├── bffi-core/               # foundation (handles, catch_unwind, ...)
│   ├── bffi-types/              # type conversion
│   ├── bffi-error/
│   ├── bffi-object/
│   ├── bffi-callback/
│   ├── bffi-class/
│   ├── bffi-dts/                # TypeScript .d.ts generation
│   ├── bffi-macros/
│   ├── bffi-event-loop/
│   ├── bffi-build/
│   └── bffi-rs/                 # public facade
├── docs/
│   ├── DESIGN.md                # architecture & decisions
│   ├── CONTRIBUTING.md
│   └── CODE_OF_CONDUCT.md
├── examples/
├── bin/                         # cli utility for bffi-rs
└── scripts/
```

New crates must follow the naming scheme `bffi-*` and be added to the workspace.

---

## 4. Development workflow

### Setup

```bash
# Rust
rustup toolchain install 1.98.0
rustup default 1.98.0

# Bun
bun install
```

### Common commands

```bash
bun run lint          # oxlint
bun run typecheck     # tsc
cargo check
cargo test
cargo fmt
cargo clippy
```

### Commit style

We use **Conventional Commits**:

```
feat: add generational handle table
fix: prevent panic across FFI boundary
docs: update DESIGN.md decisions
refactor(core): simplify catch_unwind helper
test: cover buffer copy path
chore: pin rust-toolchain to 1.98.0
```

Breaking changes must use `BREAKING CHANGE:` in the footer or `!` after the type.

### Branching and releases

- `main` - production branch; PRs into `main` are created only by the project owner, from `dev/main`.
- `dev/main` - integration branch; all feature work lands here via PRs.
- Features are developed in `dev/<feature>` branches (kebab-case), cut from and merged back into `dev/main`.
- PR `dev/<feature>` → `dev/main` requires 1 approval and a green `bun run ci`.
- Release tags `v<semver>` (annotated) are placed only on `main`, only by the owner.

Full rules: [docs/CONTRIBUTING.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/CONTRIBUTING.md) → "Branching and releases".

### Pull requests

- One logical change per PR.
- CI must pass.
- Update docs when behavior or public API changes.
- Reference related issues.

---

## 5. Rules for AI agents

When working on this repository an agent **must**:

1. Read `DESIGN.md` and this file before large changes.
2. Prefer small, reviewable diffs.
3. Never commit secrets, personal AI configs, or `.env` files.
4. Not introduce Node/Deno compatibility.
5. Keep the bottom-up architecture (small crates → `bffi-rs`).
6. Preserve the safety model (copy by default, explicit unsafe zero-copy, generational handles).
7. Run `cargo fmt`, `cargo clippy`, and tests when possible.
8. Update `DESIGN.md` or docs if a decision changes.

When unsure about architecture, prefer asking (or opening a draft PR) instead of inventing a new pattern.

---

## 6. Contact

- Issues & discussions: GitHub
- Direct contact: **contact@z2net.com**

---

## 7. Quick reference - accepted decisions

| Topic         | Decision                                     |
| ------------- | -------------------------------------------- |
| Macro         | `#[bffi]`                                    |
| Min Bun       | 1.4.0                                        |
| Rust/Cargo    | 1.98.0                                       |
| Handles       | Generational Index + type-tag                |
| Error format  | `BffiError` = code + message + source; domain errors convert losslessly via `From` |
| Boundary strings | UTF-8 canonical (`bun:ffi cstring`)          |
| Buffers       | Copy by default                              |
| Zero-copy     | Only via `bffi::unsafe_zero_copy`            |
| Event loop    | Start with `run()`, `pump()` is mock for now |
| TS types      | Generate from day one (`bffi-dts`)           |
| Panic (prod)  | Convert to JS Error                          |
| Panic (dev)   | May abort                                    |
| Compatibility | Bun only                                     |
| License       | MIT                                          |
