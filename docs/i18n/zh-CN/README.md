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

架构见 [docs/i18n/zh-CN/DESIGN.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/DESIGN.md),工程规则见 [docs/i18n/zh-CN/AGENTS.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/AGENTS.md)。

## 文档

- [docs/i18n/zh-CN/DESIGN.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/DESIGN.md) - 架构与决策
- [crates/bffi-build/CALLING-CONVENTION.md](https://github.com/DotBlood/bffi-rs/blob/main/crates/bffi-build/CALLING-CONVENTION.md) - C ABI 契约(所有跨界,含回调导出)
- [docs/i18n/zh-CN/CONTRIBUTING.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/CONTRIBUTING.md) - 贡献指南(分支、提交、PR)
- [docs/i18n/zh-CN/AGENTS.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/AGENTS.md) - 面向人类与 AI 代理的工程规则
- [packages/bffi-loader](https://github.com/DotBlood/bffi-rs/blob/main/packages/bffi-loader) - JS 运行时加载器(见其 README)
- [docs/i18n/zh-CN/SECURITY.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/SECURITY.md) - 安全策略
- [docs/i18n/zh-CN/CONTACT.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/CONTACT.md) - 联系方式

## 环境要求

- [Bun](https://bun.sh) >= 1.4.0(由 `bffi-loader` 与 `bffi` CLI 在运行时强制检查)
- Rust 1.98.0(通过 `rust-toolchain.toml` 固定;rustup 会自动安装)
- bash(commit-msg 钩子需要;macOS/Linux 预装,Windows 使用 Git Bash)

## 组成

| 部分 | 用途 |
| ---- | ---- |
| `crates/bffi-core` | 世代句柄、无锁表、catch_unwind 边界 |
| `crates/bffi-types` | 数字/字符串/缓冲区转换、SIMD UTF-8、共享 wire 编解码 |
| `crates/bffi-error` | 统一的 `BffiError` -> JS Error 映射 |
| `crates/bffi-object` | 基于全局 `Registry` 的 `ObjectWrap<T>` 所有权 |
| `crates/bffi-callback` | 双向回调 + 泛型回调 ABI |
| `crates/bffi-dts` | TypeScript IR + 确定性 `.d.ts` 渲染器 |
| `crates/bffi-macros` | `#[bffi]` / `#[bffi_async]`( shim + 描述符) |
| `crates/bffi-class` | 基于 `ObjectWrap` 的 `#[bffi_class]` / `#[bffi_impl]` |
| `crates/bffi-macro-support` | 宏 crate 共享的内部件(kinds、分类、代码生成) |
| `crates/bffi-event-loop` | JS 线程上的 `run()` / `pump()` 任务队列 |
| `crates/bffi-build` | 运行时 ABI 导出、瞬态缓冲区、`.d.ts`/loader JSON 生成器 |
| `crates/bffi-async` | `#[bffi_async]`:Rust future 变为 JS Promise(取消、超时、tokio opt-in) |
| `crates/bffi` | 门面:一个依赖覆盖整个栈 |
| `packages/bffi-loader` | 仅限 Bun 的 JS 运行时:从 loader JSON 构建类型化 API(暂未发布) |
| `bin/` | `bffi codegen` CLI |

## 快速开始

```sh
bun install             # 安装依赖 + git 钩子(lefthook)
bun run build           # 构建示例原生模块(release cdylib)
bun run codegen:native  # 生成示例的 loader JSON 与 api.gen.ts
bun run test:native     # 构建并运行 bun:ffi e2e 套件
bun run check           # oxlint + tsc + cargo check
bun run ci              # 完整 CI 对齐:lint、typecheck、fmt、clippy、测试
```

编写原生模块时,依赖 [`bffi`](https://github.com/DotBlood/bffi-rs/blob/main/crates/bffi)
(门面:一个依赖覆盖整个栈);使用属性宏时,还需依赖其展开所引用的
`bffi-core`/`bffi-types`/`bffi-dts` 等 crate。

## 生成的 TypeScript API

`#[bffi]` 描述符是唯一事实来源:同一个聚合的 `ModuleDef` 渲染出提交
到仓库的 `.d.ts`、规范的 loader JSON 与类型化的 TS 模块 - 逐字节
确定,可放心提交与 diff。

```sh
# 构建期(在你的 crate 中):
cargo run --bin emit-json                          # 写出 js/bffi.api.json
bun bffi codegen js/bffi.api.json -o js/api.gen.ts # 类型化 TS 模块
```

```ts
import { createApiFromJson } from "./api.gen.ts";

const api = createApiFromJson("./target/release/libmy.so");
api.add(1, 2);                       // number,带类型;错误抛出 JS Error
const counter = new api.counter(10); // 类:FinalizationRegistry + release()
await api.compute(21);               // `#[bffi_async]` -> Promise
```

完整示例见
[`examples/native`](https://github.com/DotBlood/bffi-rs/blob/main/examples/native)
(通过真实 `bun:ffi` 演示 shim、类、异步与回调,含 46 个测试的 e2e
对齐套件)。

## 约定

- Conventional Commits 由 commit-msg 钩子(`scripts/commit-msg.sh`)强制执行。
- pre-commit 运行 oxlint、`tsc --noEmit`、`cargo fmt --check` 和 clippy。
- pre-push 运行 workspace 测试。
- CI(GitHub Actions)已列入计划;落地之前,`bun run ci` 是唯一标准。

## 许可证

[MIT](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
