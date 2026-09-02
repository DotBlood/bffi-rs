# Contributing to bffi-rs

Thank you for your interest in contributing.

Repository: https://github.com/DotBlood/bffi-rs  
Contact: contact@z2net.com

Please also read:

- [DESIGN.md](./DESIGN.md) - architecture and decisions
- [AGENT.md](./AGENT.md) - rules for humans and AI agents
- [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md)

---

## Development setup

### Requirements

- **Rust / Cargo 1.98.0** (pinned)
- **Bun >= 1.4.0**
- Git

```bash
# Install / select Rust toolchain
rustup toolchain install 1.98.0
rustup default 1.98.0

# Clone
git clone https://github.com/DotBlood/bffi-rs.git
cd bffi-rs

# JS tooling
bun install
```

### Useful commands

```bash
bun run lint          # oxlint
bun run typecheck     # TypeScript check
cargo fmt
cargo clippy
cargo check
cargo test
```

---

## Commit message convention

We follow [Conventional Commits](https://www.conventionalcommits.org/).

Format:

```
<type>(optional scope): <short description>

[optional body]

[optional footer]
```

### Allowed types

| Type       | Meaning                                      |
|------------|----------------------------------------------|
| `feat`     | New feature                                  |
| `fix`      | Bug fix                                      |
| `docs`     | Documentation only                           |
| `style`    | Formatting, no logic change                  |
| `refactor` | Code change that is not a fix or feature     |
| `perf`     | Performance improvement                      |
| `test`     | Adding or fixing tests                       |
| `build`    | Build system or dependencies                 |
| `ci`       | CI configuration                             |
| `chore`    | Maintenance tasks                            |
| `revert`   | Revert a previous commit                     |

### Examples

```
feat(core): implement generational handle table
fix(types): correctly copy UTF-8 strings across FFI
docs: clarify zero-copy policy in DESIGN.md
refactor(macros): simplify #[bffi] expansion
chore: pin rust-toolchain to 1.98.0
```

Breaking changes:

```
feat(api)!: rename Handle to BffiHandle

BREAKING CHANGE: the public type `Handle` was renamed to `BffiHandle`.
```

---

## Pull requests

1. Fork the repository and create a branch from `main`.
2. Make a focused change (one logical unit per PR).
3. Ensure `cargo fmt`, `cargo clippy`, tests, and `bun run lint` pass.
4. Fill in the pull request template.
5. Link related issues if any.
6. Be ready to discuss design decisions - we care about long-term consistency with `DESIGN.md`.

### PR title

Prefer the same Conventional Commits style as commit messages.

---

## Issues

Use the provided issue templates:

- **Bug report**
- **Feature request**
- **Design / architecture question**

When reporting a bug, include:

- Bun version (`bun --version`)
- Rust version (`rustc --version`)
- OS and architecture
- Minimal reproduction if possible

---

## Safety and architecture reminders

- Default data path is **copy**, not zero-copy.
- Zero-copy only through `bffi::unsafe_zero_copy`.
- Handles are Generational Index + type-tag.
- No Node.js / Deno compatibility code.
- Do not commit secrets, personal AI config folders (`.grok`, `.claude`, `.codex`, …), or `.env` files.

---

## Code of conduct

Be respectful. See [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md).

Harassment, toxic behavior, or bad-faith contributions will not be tolerated.

---

## License

By contributing you agree that your contributions are licensed under the **MIT** license.

---

## Questions?

- Open a GitHub Discussion or Issue
- Email: **contact@z2net.com**
