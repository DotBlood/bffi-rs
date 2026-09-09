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
- [crates/bffi-build/CALLING-CONVENTION.md](https://github.com/DotBlood/bffi-rs/blob/main/crates/bffi-build/CALLING-CONVENTION.md) - контракт C ABI (все пересечения границы, включая колбэк-экспорты)
- [docs/i18n/ru/CONTRIBUTING.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/CONTRIBUTING.md) - как внести вклад (ветки, коммиты, PR)
- [docs/i18n/ru/AGENTS.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/AGENTS.md) - правила инженерии для людей и AI-агентов
- [packages/bffi-loader](https://github.com/DotBlood/bffi-rs/blob/main/packages/bffi-loader) - JS-рантайм-лоадер (см. его README)
- [docs/i18n/ru/SECURITY.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/SECURITY.md) - политика безопасности
- [docs/i18n/ru/CONTACT.md](https://github.com/DotBlood/bffi-rs/blob/main/docs/i18n/ru/CONTACT.md) - контакты

## Требования

- [Bun](https://bun.sh) >= 1.4.0 (проверяется в рантайме `bffi-loader` и CLI `bffi`)
- Rust 1.98.0 (закреплён в `rust-toolchain.toml`; rustup установит его автоматически)
- bash (для commit-msg хука; предустановлен на macOS/Linux, Git Bash на Windows)

## Состав

| Часть | Назначение |
| ----- | ---------- |
| `crates/bffi-core` | Генерационные хендлы, lock-free таблицы, граница catch_unwind |
| `crates/bffi-types` | Конвертация чисел/строк/буферов, SIMD UTF-8, общий wire-кодек |
| `crates/bffi-error` | Единый маппинг `BffiError` -> JS Error |
| `crates/bffi-object` | Владение `ObjectWrap<T>` поверх глобального `Registry` |
| `crates/bffi-callback` | Колбэки в оба направления + генерический callback ABI |
| `crates/bffi-dts` | TypeScript IR + детерминированный рендер `.d.ts` |
| `crates/bffi-macros` | `#[bffi]` / `#[bffi_async]` (шимы + дескрипторы) |
| `crates/bffi-class` | `#[bffi_class]` / `#[bffi_impl]` поверх `ObjectWrap` |
| `crates/bffi-macro-support` | Общие внутренности макросов (kinds, классификация, кодеген) |
| `crates/bffi-event-loop` | Очередь задач `run()` / `pump()` на JS-потоке |
| `crates/bffi-build` | Runtime ABI-экспорты, транзитные буферы, эмиттеры `.d.ts`/loader JSON |
| `crates/bffi-async` | `#[bffi_async]`: Rust-фьючерсы как JS Promises (отмена, таймауты, tokio opt-in) |
| `crates/bffi` | Фасад: одна зависимость на весь стек |
| `packages/bffi` | Bun-only JS-интеграция: конфиг, полный пайплайн (build → json → api.gen), типизированный лоадер (`@z2net/bffi`, публикация pending) |
| `packages/bffi-cli` | CLI `bffi`: init, build, codegen, pack, fetch, check, doctor (`@z2net/bffi-cli`, публикация pending) |
| `packages/native-template` | Шаблон COPY-ME: pattern-A npm-упаковка нативного модуля (reference-only) |

## Начало работы

```sh
bun install             # устанавливает зависимости + git-хуки (lefthook)
bun run build           # собирает пример нативного модуля (release cdylib)
bun run codegen:native  # эмитит loader JSON и генерирует api.gen.ts примера
bun run test:native     # собирает его и запускает e2e-сьют через bun:ffi
bun run check           # oxlint + tsc + cargo check
bun run ci              # полный CI-паритет: lint, typecheck, fmt, clippy, тесты
```

Для нативного модуля зависите от [`bffi`](https://github.com/DotBlood/bffi-rs/blob/main/crates/bffi)
(фасад: одна зависимость на весь стек) и - при использовании атрибутных
макросов - от отдельных крейтов `bffi-core`/`bffi-types`/`bffi-dts`,
которые называют их раскрытия.

## Генерируемый TypeScript API

Дескрипторы `#[bffi]` - единый источник истины: один и тот же
агрегированный `ModuleDef` рендерит закоммиченный `.d.ts`, канонический
loader JSON и типизированный TS-модуль - побайтово детерминированно,
безопасно коммитить и диффать.

```sh
# build-time (в вашем крейте):
cargo run --bin emit-json                          # пишет js/bffi.api.json
bun bffi codegen js/bffi.api.json -o js/api.gen.ts # типизированный TS-модуль
```

```ts
import { createApiFromJson } from "./api.gen.ts";

const api = createApiFromJson("./target/release/libmy.so");
api.add(1, 2);                       // number, типизировано; ошибки - JS Error
const counter = new api.counter(10); // классы: FinalizationRegistry + release()
await api.compute(21);               // `#[bffi_async]` -> Promise
```

Полный рабочий пример - в
[`examples/native`](https://github.com/DotBlood/bffi-rs/blob/main/examples/native)
(шимы, классы, async и колбэки через настоящий `bun:ffi`, e2e-сьют
паритета на 46 тестов).

## Конвенции

- Conventional Commits контролируются commit-msg хуком (`scripts/commit-msg.sh`).
- pre-commit запускает oxlint, `tsc --noEmit`, `cargo fmt --check` и clippy.
- pre-push запускает тесты workspace.
- CI (GitHub Actions) запланирован; пока он не заведён, источник истины - `bun run ci`.

## Лицензия

[MIT](https://github.com/DotBlood/bffi-rs/blob/main/LICENSE)
