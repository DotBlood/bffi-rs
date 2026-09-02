# 安全策略

[English](https://github.com/DotBlood/bffi-rs/blob/main/SECURITY.md) | [Русский](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/SECURITY.md) | **[简体中文](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/SECURITY.md)**

## 报告漏洞

邮箱:**contact@z2net.com**

请包含:
- 问题描述
- 复现步骤
- 潜在影响
- 受影响的版本

我们的目标是在 72 小时内回复。在修复发布之前,请勿公开披露。

## 范围

- FFI 边界中的内存安全缺陷
- 句柄表损坏 / 类型混淆
- panic 传播问题
- `bffi-macros` 中可能隐藏 `unsafe` 的代码生成缺陷

不在范围内:
- Bun 本身的缺陷(请向 oven-sh/bun 报告)
- Rust 工具链的缺陷
