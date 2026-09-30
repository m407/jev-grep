# Установка и конфигурация

[English](../installation.md) | **Русский** · [Документация](README.md)

## Установка

Из корня репозитория:

```sh
cargo install --path . --locked
jev-grep --help
```

Нужен Rust с поддержкой edition 2024; сборка проверена на Rust 1.98.1.
Зависимости закреплены в `Cargo.lock`. Python/PyTorch не нужны самому клиенту.

## Подключение модели

Для настройки fnox/keychain см. [руководство по подключению Zen](deployment.md).
По умолчанию `jev-grep` обращается к `https://opencode.ai/zen/v1/systemone`
с моделью `jev-1.13`. Автоматического переключения endpoint или модели нет.

```sh
export JEV_GREP_API_URL=https://opencode.ai/zen/v1/systemone
export JEV_GREP_MODEL=jev-1.13

# Сначала настройте ключ через fnox; см. руководство по подключению.
fnox exec -- jev-grep 'Certificate validation failure' application.log
```

Endpoint и модель также можно передать флагами `--api-url` и `--model`.
Язык критерия и входа должен поддерживаться выбранной моделью.
Переопределения используют `JEV_GREP_API_URL`, `JEV_GREP_MODEL`
и `JEV_GREP_API_KEY`: флаги имеют приоритет над переменными окружения,
а переменные — над значениями по умолчанию.

Далее: [выбор режима входа](README.md#выбор-режима-входа),
[поиск в логах](logs.md) или [поиск кода](code-search.md).
