# bffi-rs

Binding framework for Bun — a napi-rs-equivalent for [Bun](https://bun.sh), built on `bun:ffi` and a thin C ABI. Written in Rust, bottom-up from small focused crates.

See [docs/DESIGN.md](docs/DESIGN.md) for architecture and [AGENT.md](AGENT.md) for the project's engineering rules.

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

[MIT](LICENSE)
