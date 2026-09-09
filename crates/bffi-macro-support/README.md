# bffi-macro-support

[![License: MIT](https://img.shields.io/badge/License-MIT-3DA639)](https://github.com/z2net/bffi-rs/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.98.0-DEA584?logo=rust&logoColor=white)](https://github.com/z2net/bffi-rs/blob/main/rust-toolchain.toml)

The shared macro-internals crate of
[bffi-rs](https://github.com/z2net/bffi-rs/blob/main/README.md) - the
Bun-only native binding framework. It removes the duplication between the two
proc-macro crates, `bffi-macros` (`#[bffi]`) and `bffi-class`
(`#[bffi_class]`/`#[bffi_impl]`), so the boundary mapping cannot drift between
them.

**Status:** P3 tooling crate. **No runtime code, no ABI**: nothing here runs
in the shipped native library - it is compile-time support consumed only by
the proc-macro crates. It is NOT re-exported through the `bffi` facade and is
not part of any user-facing contract.

---

## Contents

| Module          | Provides                                                                                     |
| --------------- | -------------------------------------------------------------------------------------------- |
| [`kind`]        | The boundary kind model: `PrimTy`, `BigIntTy`, `ShimKind`, `BufferTy`, `RetKind`, `TsKind`   |
| [`classify`]    | `syn::Type` -> kind classification plus the `TsType` mapping, with neutral rejections         |
| [`codegen`]     | Shim transport token generators: out-parameters, return tails, cstring conversions            |
| [`diagnostics`] | `MacroDiagnostic` (code + message + help/notes -> `compile_error!`) and the `DESIGN_NOTE`     |
| [`util`]        | `extract_docs` and `to_snake_case` - small parse helpers                                     |

---

## What lives here vs. what stays with the consumers

The split is deliberate and behavior-preserving:

| Concern                                              | Home                                       |
| ---------------------------------------------------- | ------------------------------------------ |
| Attribute entry points (`#[bffi]`, `#[bffi_class]`)  | `bffi-macros` / `bffi-class`               |
| Item parsing (`ItemFn` vs `ItemStruct`/`ItemImpl`)   | `bffi-macros` / `bffi-class`               |
| Diagnostics constructors with E-codes and exact texts | `bffi-macros` (`E001`-`E004`) / `bffi-class` (`E005`-`E008`) |
| Boundary kind model, classification, codegen         | `bffi-macro-support` (this crate)          |
| Diagnostic renderer (formatting only)                | `bffi-macro-support` (this crate)          |

The per-crate error texts are locked by the trybuild golden `.stderr` files in
each proc-macro crate; do not renumber either E-series.

## Neutral rejections

The classifiers do not format errors. A rejected type comes back as an
`Unsupported { span, ty }` value, and each proc-macro crate maps it onto its
own diagnostic - `bffi[E002]` for `#[bffi]` parameters versus `bffi[E007]`
for class members, for example - keeping every published message
byte-identical to the pre-extraction texts.

## Testing

```sh
cargo test -p bffi-macro-support
```

Suites: unit tests for the kind tokens, the classifiers (accepted and
rejected matrices), the codegen token generators, the diagnostic renderer,
and the parse helpers. The proc-macro crates run their own full acceptance
suites (shim integration, descriptors, trybuild UI goldens) against this
crate.

## Requirements

- Rust **1.98.0** (pinned workspace-wide via `rust-toolchain.toml`)
- No Bun and no other runtime is needed to use or test this crate

## License

MIT - see [LICENSE](https://github.com/z2net/bffi-rs/blob/main/LICENSE).
