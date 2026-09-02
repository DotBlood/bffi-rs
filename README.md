# bffi-rs

<div align="center">

[![Bun](https://img.shields.io/badge/Bun-%3E%3D1.4.0-F472B6?logo=bun&logoColor=white)](https://bun.sh)
[![Rust](https://img.shields.io/badge/Rust-1.98.0-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-3DA639?logo=opensourceinitiative&logoColor=white)](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
[![GitHub Issues](https://img.shields.io/github/issues/DotBlood/bffi-rs)](https://github.com/DotBlood/bffi-rs/issues)
[![GitHub Pull Requests](https://img.shields.io/github/issues-pr/DotBlood/bffi-rs)](https://github.com/DotBlood/bffi-rs/pulls)

**English** | [Русский](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/README.md) | [简体中文](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/README.md)

</div>

Binding framework for Bun - a napi-rs-equivalent for [Bun](https://bun.sh), built on `bun:ffi` and a thin C ABI. Written in Rust, bottom-up from small focused crates.

See [docs/DESIGN.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/DESIGN.md) for architecture and [AGENT.md](https://github.com/DotBlood/bffi-rs/blob/main/AGENT.md) for the project's engineering rules.

## Documentation

- [docs/DESIGN.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/DESIGN.md) - architecture & decisions
- [docs/CONTRIBUTING.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/CONTRIBUTING.md) - how to contribute (branching, commits, PRs)
- [AGENT.md](https://github.com/DotBlood/bffi-rs/blob/main/AGENT.md) - engineering rules for humans and AI agents
- [SECURITY.md](https://github.com/DotBlood/bffi-rs/blob/main/SECURITY.md) - security policy
- [CONTACT.md](https://github.com/DotBlood/bffi-rs/blob/main/CONTACT.md) - contacts

## Requirements

- [Bun](https://bun.sh) >= 1.4.0
- Rust 1.98.0 (pinned via `rust-toolchain.toml`; rustup installs it automatically)
- bash (for the commit-msg hook; preinstalled on macOS/Linux, Git Bash on Windows)

## Getting started

```sh
bun install          # installs dependencies + git hooks (lefthook)
bun run build        # TODO: cargo build wiring
bun run check        # oxlint + tsc + cargo check
bun run ci           # full CI parity: lint, typecheck, fmt, clippy, tests
```

## Conventions

- Conventional Commits are enforced by a `commit-msg` hook (`scripts/commit-msg.sh`).
- Pre-commit runs oxlint, `tsc --noEmit`, `cargo fmt --check` and clippy.
- Pre-push runs the workspace tests.
- CI (GitHub Actions) is planned and will be introduced once `bffi-core` lands; until then `bun run ci` is the source of truth.

## License

[MIT](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
