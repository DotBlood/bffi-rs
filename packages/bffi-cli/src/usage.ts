/** The top-level usage text (printed for --help and usage errors). */
export const USAGE = `bffi - CLI for bffi-rs native modules (Bun-only)

Usage:
  bffi init     [--root <dir>] [--module <m>] [--crate-dir <d>] [--crate-name <n>] [--binary <b>]
  bffi build    [--config <p>] [--skip-build] [--debug]
  bffi codegen  <input.json> -o <out.ts> [--runtime <module>]
  bffi pack     --src <binary> --triple <t> [--name <base>] [--out <dir>]
  bffi fetch    --repo <owner/repo> --tag <v> --asset <file> [--out <dir>]

Commands:
  init     scaffold .bffi/bffi.json + a minimal Rust crate
  build    full pipeline: cargo build -> loader JSON -> api.gen -> dlopen check
  codegen  render a typed TS module from a loader JSON
  pack     assemble a platform npm package from a built binary
  fetch    download a prebuilt binary from a GitHub Release (+ sha256)

Planned (v2): doctor, check.

Exit codes: 0 = ok, 1 = usage error, 2 = input/environment failure.
`;
