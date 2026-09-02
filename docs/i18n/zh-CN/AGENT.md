# AGENT.md - AI 代理与贡献者规则

[English](https://github.com/DotBlood/bffi-rs/blob/main/AGENT.md) | [Русский](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/AGENT.md) | **[简体中文](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/AGENT.md)**

本文件定义了人类与 AI 代理在 **bffi-rs** 上工作时必须遵循的规则。

仓库:https://github.com/DotBlood/bffi-rs  
联系方式:contact@z2net.com

---

## 1. 项目目的

`bffi-rs` 是一个用 Rust 编写的**仅支持 Bun** 的原生绑定框架。

它是 `napi-rs` 在 Bun 上的等价物,但:

- 仅面向 **Bun**(无 Node.js / Deno 兼容性);
- **不**依赖 Node-API;
- 使用 `bun:ffi` 和一层薄的 C ABI 层;
- 自**底向上**由小型 crate 构建而成。

首要目标:FFI 边界的安全性、清晰的所有权、良好的 DX 以及长期可维护性。

在进行架构更改之前,请先阅读 `DESIGN.md`。

---

## 2. 硬性规则

1. **仅支持 Bun**  
   不要添加 Node.js 或 Deno 兼容层。

2. **安全第一**
   - 默认路径 = 复制数据,绝不零拷贝。
   - 仅允许通过 `bffi::unsafe_zero_copy` 进行零拷贝。
   - 所有 `extern "C"` 函数必须保持精简,并用 `catch_unwind` 包裹。

3. **句柄**  
   使用 Generational Index + type-tag(`u64`)。  
   绝不通过 C ABI 暴露原始 Rust 引用或复杂类型。

4. **Panic**
   - 开发构建可以中止(更易于调试)。
   - 生产构建必须将 panic 转换为 JS `Error`。

5. **最低 Bun 版本**  
   `1.4.0`

6. **Rust / Cargo 版本**  
   项目锁定为 **Cargo / Rust 1.98.0**。  
   未经明确决策和 CI 更新,不得提升版本。

7. **仓库中不得包含机密信息**  
   任何位于 `.grok`、`.claude`、`.codex`、`.opencode`、`.hermes`、`.mcp`、`.env` 下的内容,以及密钥、令牌等,都必须排除在 git 之外(参见 `.gitignore`)。

8. **许可证**  
   MIT。在适当之处保留 SPDX 头部声明。

---

## 3. 仓库结构

```
bffi-rs/
├── AGENT.md                    # this file
├── README.md
├── LICENSE
├── SECURITY.md
├── CONTACT.md
├── Cargo.toml                  # workspace
├── rust-toolchain.toml         # pinned 1.98.0
├── package.json                # Bun workspace / scripts
├── tsconfig.json
├── .oxlintrc.json              # linter config
├── lefthook.yml                # git hooks (lint, fmt, commit-msg)
├── .gitignore
├── .github/
│   ├── ISSUE_TEMPLATE/
│   ├── PULL_REQUEST_TEMPLATE.md
│   └── workflows/              # CI (planned; deferred until bffi-core lands)
├── crates/
│   ├── bffi-core/               # foundation (handles, catch_unwind, ...)
│   ├── bffi-types/              # type conversion
│   ├── bffi-error/
│   ├── bffi-object/
│   ├── bffi-callback/
│   ├── bffi-class/
│   ├── bffi-dts/                # TypeScript .d.ts generation
│   ├── bffi-macros/
│   ├── bffi-event-loop/
│   ├── bffi-build/
│   └── bffi-rs/                 # public facade
├── docs/
│   ├── DESIGN.md                # architecture & decisions
│   ├── CONTRIBUTING.md
│   └── CODE_OF_CONDUCT.md
├── examples/
├── bin/                         # cli utility for bffi-rs
└── scripts/
```

新的 crate 必须遵循 `bffi-*` 命名方案,并被添加到 workspace 中。

---

## 4. 开发工作流

### 环境配置

```bash
# Rust
rustup toolchain install 1.98.0
rustup default 1.98.0

# Bun
bun install
```

### 常用命令

```bash
bun run lint          # oxlint
bun run typecheck     # tsc
cargo check
cargo test
cargo fmt
cargo clippy
```

### 提交风格

我们使用 **Conventional Commits**:

```
feat: add generational handle table
fix: prevent panic across FFI boundary
docs: update DESIGN.md decisions
refactor(core): simplify catch_unwind helper
test: cover buffer copy path
chore: pin rust-toolchain to 1.98.0
```

破坏性变更必须在页脚使用 `BREAKING CHANGE:`,或在类型之后加上 `!`。

### 分支与发布

- `main` - 生产分支;进入 `main` 的 PR 仅能由项目所有者从 `dev/main` 创建。
- `dev/main` - 集成分支;所有功能工作都通过 PR 汇入此处。
- 功能在 `dev/<feature>` 分支(kebab-case)中开发,从 `dev/main` 切出并合并回 `dev/main`。
- PR `dev/<feature>` → `dev/main` 需要 1 个批准以及绿色的 `bun run ci`。
- 发布标签 `v<semver>`(附注标签)仅放置在 `main` 上,且仅由所有者创建。

完整规则:[docs/CONTRIBUTING.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/CONTRIBUTING.md) → “分支与发布”。

### 拉取请求

- 每个 PR 只包含一个逻辑变更。
- CI 必须通过。
- 当行为或公共 API 发生变化时,更新文档。
- 引用相关的 issue。

---

## 5. AI 代理规则

在此仓库上工作时,AI 代理**必须**:

1. 在进行大规模更改之前,先阅读 `DESIGN.md` 和本文件。
2. 优先采用小的、易于审查的 diff。
3. 绝不提交机密信息、个人 AI 配置或 `.env` 文件。
4. 不引入 Node/Deno 兼容性。
5. 保持自底向上的架构(小型 crate → `bffi-rs`)。
6. 保持安全模型(默认复制、显式的 unsafe 零拷贝、代际句柄)。
7. 在可能的情况下运行 `cargo fmt`、`cargo clippy` 和测试。
8. 如果某项决策发生变化,更新 `DESIGN.md` 或相关文档。

对架构拿不准时,优先选择询问(或打开一个 draft PR),而不是发明新模式。

---

## 6. 联系方式

- Issue 与讨论:GitHub
- 直接联系:**contact@z2net.com**

---

## 7. 快速参考 - 已接受的决策

| 主题          | 决策                                         |
| ------------- | -------------------------------------------- |
| 宏            | `#[bffi]`                                    |
| 最低 Bun      | 1.4.0                                        |
| Rust/Cargo    | 1.98.0                                       |
| 句柄          | Generational Index + type-tag                |
| 缓冲区        | 默认复制                                     |
| 零拷贝        | 仅通过 `bffi::unsafe_zero_copy`              |
| 事件循环      | 以 `run()` 启动,`pump()` 目前为 mock 实现   |
| TS 类型       | 从第一天起生成(`bffi-dts`)                  |
| Panic(生产)  | 转换为 JS Error                              |
| Panic(开发)  | 可以中止                                     |
| 兼容性        | 仅支持 Bun                                   |
| 许可证        | MIT                                          |
