# Security Policy

## Reporting a Vulnerability

Email: **contact@z2net.com**

Please include:
- Description of the issue
- Steps to reproduce
- Potential impact
- Affected version

We aim to respond within 72 hours. Please do not disclose publicly until a fix is released.

## Scope

- Memory safety bugs in the FFI boundary
- Handle table corruption / type confusion
- Panic propagation issues
- Codegen bugs in `bffi-macros` that could hide `unsafe`

Out of scope:
- Bugs in Bun itself (report to oven-sh/bun)
- Bugs in Rust toolchain
