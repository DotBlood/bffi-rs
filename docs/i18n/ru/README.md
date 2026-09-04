# bffi-rs

<div align="center">

[![Bun](https://img.shields.io/badge/Bun-%3E%3D1.4.0-F472B6?logo=bun&logoColor=white)](https://bun.sh)
[![Rust](https://img.shields.io/badge/Rust-1.98.0-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-3DA639?logo=opensourceinitiative&logoColor=white)](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
[![GitHub Issues](https://img.shields.io/github/issues/DotBlood/bffi-rs)](https://github.com/DotBlood/bffi-rs/issues)
[![GitHub Pull Requests](https://img.shields.io/github/issues-pr/DotBlood/bffi-rs)](https://github.com/DotBlood/bffi-rs/pulls)

[English](https://github.com/DotBlood/bffi-rs/blob/main/README.md) | **Русский** | [简体中文](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/README.md)

</div>

Фреймворк биндингов для Bun - аналог napi-rs для [Bun](https://bun.sh), построенный на `bun:ffi` и тонком C ABI. Написан на Rust, развивается снизу вверх из небольших сфокусированных крейтов.

Архитектура описана в [docs/i18n/ru/DESIGN.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/DESIGN.md), инженерные правила проекта - в [docs/i18n/ru/AGENTS.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/AGENTS.md).

## Документация

- [docs/i18n/ru/DESIGN.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/DESIGN.md) - архитектура и решения
- [docs/i18n/ru/CONTRIBUTING.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/CONTRIBUTING.md) - как внести вклад (ветки, коммиты, PR)
- [docs/i18n/ru/AGENTS.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/AGENTS.md) - правила инженерии для людей и AI-агентов
- [docs/i18n/ru/SECURITY.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/SECURITY.md) - политика безопасности
- [docs/i18n/ru/CONTACT.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/CONTACT.md) - контакты

## Требования

- [Bun](https://bun.sh) >= 1.4.0
- Rust 1.98.0 (закреплён в `rust-toolchain.toml`; rustup установит его автоматически)
- bash (для commit-msg хука; предустановлен на macOS/Linux, Git Bash на Windows)

## Начало работы

```sh
bun install          # устанавливает зависимости + git-хуки (lefthook)
bun run build        # TODO: подключение cargo build
bun run check        # oxlint + tsc + cargo check
bun run ci           # полный CI-паритет: lint, typecheck, fmt, clippy, тесты
```

## Конвенции

- Conventional Commits контролируются commit-msg хуком (`scripts/commit-msg.sh`).
- pre-commit запускает oxlint, `tsc --noEmit`, `cargo fmt --check` и clippy.
- pre-push запускает тесты workspace.
- CI (GitHub Actions) запланирован и появится, когда будет готов `bffi-core`; до этого источник истины - `bun run ci`.

## Лицензия

[MIT](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
