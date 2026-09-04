# AGENTS.md - Правила для AI-агентов и контрибьюторов

[English](https://github.com/DotBlood/bffi-rs/blob/main/AGENTS.md) | **[Русский](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/AGENTS.md)** | [简体中文](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/zh-CN/AGENTS.md)

Этот файл определяет, как люди и AI-агенты должны работать над **bffi-rs**.

Репозиторий: https://github.com/DotBlood/bffi-rs
Контакт: contact@z2net.com

---

## 1. Назначение проекта

`bffi-rs` - это **Bun-only** фреймворк биндингов, написанный на Rust.

Это Bun-эквивалент `napi-rs`, но:

- нацелен **только на Bun** (без совместимости с Node.js / Deno);
- **не** зависит от Node-API;
- использует `bun:ffi` и тонкий слой C ABI;
- строится **снизу вверх** из небольших крейтов.

Основные цели: безопасность на границе FFI, ясная модель владения, хороший DX и долгосрочная сопровождаемость.

Читайте `DESIGN.md` перед внесением архитектурных изменений.

---

## 2. Жёсткие правила

1. **Только Bun**
   Не добавляйте слои совместимости с Node.js или Deno.

2. **Безопасность прежде всего**
   - Путь по умолчанию = копирование данных, никогда zero-copy.
   - Zero-copy разрешён только через `bffi::unsafe_zero_copy`.
   - Все функции `extern "C"` должны быть тонкими и обёрнутыми в `catch_unwind`.

3. **Хендлы**
   Используйте Generational Index + type-tag (`u64`).
   Никогда не выставляйте наружу сырые ссылки Rust или сложные типы через C ABI.

4. **Паники**
   - Dev-сборки могут прерываться (проще отлаживать).
   - Prod-сборки должны преобразовывать паники в JS `Error`.

5. **Минимальная версия Bun**
   `1.4.0`

6. **Версия Rust / Cargo**
   Проект зафиксирован на **Cargo / Rust 1.98.0**.
   Не поднимайте версию без явного решения и обновления CI.

7. **Никаких секретов в репозитории**
   Всё, что находится под `.grok`, `.claude`, `.codex`, `.opencode`, `.hermes`, `.mcp`, `.env`, ключи, токены и т.п., должно оставаться вне git (см. `.gitignore`).

8. **Лицензия**
   MIT. Сохраняйте SPDX-заголовки там, где это уместно.

---

## 3. Структура репозитория

```
bffi-rs/
├── AGENTS.md                    # this file
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

Новые крейты должны следовать схеме именования `bffi-*` и добавляться в workspace.

---

## 4. Процесс разработки

### Настройка

```bash
# Rust
rustup toolchain install 1.98.0
rustup default 1.98.0

# Bun
bun install
```

### Часто используемые команды

```bash
bun run lint          # oxlint
bun run typecheck     # tsc
cargo check
cargo test
cargo fmt
cargo clippy
```

### Стиль коммитов

Мы используем **Conventional Commits**:

```
feat: add generational handle table
fix: prevent panic across FFI boundary
docs: update DESIGN.md decisions
refactor(core): simplify catch_unwind helper
test: cover buffer copy path
chore: pin rust-toolchain to 1.98.0
```

Критические изменения (breaking changes) должны указываться через `BREAKING CHANGE:` в футере или `!` после типа.

### Ветвление и релизы

- `main` - продакшн-ветка; пул-реквесты в `main` создаёт только владелец проекта, из `dev/main`.
- `dev/main` - интеграционная ветка; вся работа над фичами попадает сюда через пул-реквесты.
- Фичи разрабатываются в ветках `dev/<feature>` (kebab-case), которые отпочковываются от `dev/main` и мержатся обратно в `dev/main`.
- Пул-реквест `dev/<feature>` → `dev/main` требует 1 одобрения и зелёного `bun run ci`.
- Релизные теги `v<semver>` (аннотированные) ставятся только на `main` и только владельцем.

Полные правила: [docs/i18n/ru/CONTRIBUTING.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/CONTRIBUTING.md) → "Ветвление и релизы".

### Пул-реквесты

- Одно логическое изменение на пул-реквест.
- CI должен проходить.
- Обновляйте документацию при изменении поведения или публичного API.
- Ссылайтесь на связанные issues.

---

## 5. Правила для AI-агентов

Работая над этим репозиторием, агент **обязан**:

1. Прочитать `DESIGN.md` и этот файл перед крупными изменениями.
2. Предпочитать небольшие, удобные для ревью диффы.
3. Никогда не коммитить секреты, личные AI-конфигурации или файлы `.env`.
4. Не вводить совместимость с Node/Deno.
5. Сохранять архитектуру снизу вверх (небольшие крейты → `bffi-rs`).
6. Сохранять модель безопасности (копирование по умолчанию, явный unsafe zero-copy, generational-хендлы).
7. Запускать `cargo fmt`, `cargo clippy` и тесты, когда это возможно.
8. Обновлять `DESIGN.md` или документацию, если меняется решение.

Если возникают сомнения насчёт архитектуры, лучше спросить (или открыть черновой PR), чем изобретать новый паттерн.

---

## 6. Контакт

- Issues и обсуждения: GitHub
- Прямой контакт: **contact@z2net.com**

---

## 7. Краткий справочник - принятые решения

| Тема          | Решение                                 |
| ------------- | --------------------------------------- |
| Макрос        | `#[bffi]`                               |
| Мин. Bun      | 1.4.0                                   |
| Rust/Cargo    | 1.98.0                                  |
| Хендлы        | Generational Index + type-tag           |
| Формат ошибок | `BffiError` = код + сообщение + источник; доменные ошибки конвертируются без потерь через `From` |
| Строки на границе | UTF-8 каноническая (`bun:ffi cstring`)  |
| Таблицы       | Lock-free; reclamation через hazard     |
| UTF-8 проверка| SIMD (x86 SSSE3, aarch64 NEON)          |
| Буферы        | Копирование по умолчанию                |
| Zero-copy     | Только через `bffi::unsafe_zero_copy`   |
| Event loop    | Старт с `run()`, `pump()` пока заглушка |
| TS-типы       | Генерация с первого дня (`bffi-dts`)    |
| Паника (prod) | Преобразуется в JS Error                |
| Паника (dev)  | Может прерываться (abort)               |
| Совместимость | Только Bun                              |
| Лицензия      | MIT                                     |
