//! # bffi-dts
//!
//! TypeScript `.d.ts` generation for the bffi-rs framework: native
//! bindings for [Bun](https://bun.sh). Types are generated from day
//! one so every exported symbol ships with an accurate declaration
//! file (see the "TS types" decision in
//! [DESIGN.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/DESIGN.md)).
//!
//! ## Mission
//!
//! - A static intermediate representation (IR) of declarations.
//!   Descriptors are `&'static` values so the `#[bffi]` macro
//!   (a later stage) can emit them as constants.
//! - A deterministic renderer: the output depends only on the IR.
//!   Re-rendering the same IR always produces a byte-identical file.
//! - Exported symbols follow the `export_name = "bffi_" + js_name`
//!   convention, so declarations match the C ABI surface exactly.
//!
//! ## Render guarantees
//!
//! The renderer is pure and never panics:
//!
//! - a fixed two-line header; no timestamps or versions embedded;
//! - LF line endings and a trailing newline;
//! - output is a function of the IR alone.

#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]
