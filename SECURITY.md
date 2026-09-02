# Security Policy

[English](https://github.com/DotBlood/bffi-rs/blob/main/SECURITY.md) | [Русский](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/SECURITY.md) | [简体中文](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/SECURITY.md)

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
