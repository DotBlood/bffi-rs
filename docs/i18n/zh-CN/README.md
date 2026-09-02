# bffi-rs

<div align="center">

[![Bun](https://img.shields.io/badge/Bun-%3E%3D1.4.0-F472B6?logo=bun&logoColor=white)](https://bun.sh)
[![Rust](https://img.shields.io/badge/Rust-1.98.0-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-3DA639?logo=opensourceinitiative&logoColor=white)](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
[![GitHub Issues](https://img.shields.io/github/issues/DotBlood/bffi-rs)](https://github.com/DotBlood/bffi-rs/issues)
[![GitHub Pull Requests](https://img.shields.io/github/issues-pr/DotBlood/bffi-rs)](https://github.com/DotBlood/bffi-rs/pulls)

[English](https://github.com/DotBlood/bffi-rs/blob/main/README.md) | [Русский](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/README.md) | **简体中文**

</div>

Bun 绑定框架 - napi-rs 的 Bun 等价物,基于 `bun:ffi` 与轻量 C ABI 构建。使用 Rust 编写,自底向上由多个小型专用 crate 组成。

架构见 [docs/i18n/zh-CN/DESIGN.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/DESIGN.md),工程规则见 [docs/i18n/zh-CN/AGENT.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/AGENT.md)。

## 文档

- [docs/i18n/zh-CN/DESIGN.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/DESIGN.md) - 架构与决策
- [docs/i18n/zh-CN/CONTRIBUTING.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/CONTRIBUTING.md) - 贡献指南(分支、提交、PR)
- [docs/i18n/zh-CN/AGENT.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/AGENT.md) - 面向人类与 AI 代理的工程规则
- [docs/i18n/zh-CN/SECURITY.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/SECURITY.md) - 安全策略
- [docs/i18n/zh-CN/CONTACT.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/CONTACT.md) - 联系方式

## 环境要求

- [Bun](https://bun.sh) >= 1.4.0
- Rust 1.98.0(通过 `rust-toolchain.toml` 固定;rustup 会自动安装)
- bash(commit-msg 钩子需要;macOS/Linux 预装,Windows 使用 Git Bash)

## 快速开始

```sh
bun install          # 安装依赖 + git 钩子(lefthook)
bun run build        # TODO: 接入 cargo build
bun run check        # oxlint + tsc + cargo check
bun run ci           # 完整 CI 对齐:lint、typecheck、fmt、clippy、测试
```

## 约定

- Conventional Commits 由 commit-msg 钩子(`scripts/commit-msg.sh`)强制执行。
- pre-commit 运行 oxlint、`tsc --noEmit`、`cargo fmt --check` 和 clippy。
- pre-push 运行 workspace 测试。
- CI(GitHub Actions)计划在 `bffi-core` 完成后引入;在此之前 `bun run ci` 是唯一标准。

## 许可证

[MIT](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
