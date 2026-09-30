# Проверки для разработчиков

[English](../development.md) | **Русский** · [Документация](README.md)

Запустите из корня репозитория:

```sh
cargo test --locked
cargo build --locked
python3 -m unittest discover -s tests -v
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

Acceptance-тесты используют Python standard library и локальный mock HTTP API.
Они проверяют группировку при заданных модельных ответах, форматы входа/вывода,
координаты, лимиты, фильтры и ошибки. Сервер Jev on OpenCode Zen и веса не требуются;
точность настоящей модели эти тесты не измеряют.

Автоматические релизы из `main`, Conventional Commits и проверки release tooling
описаны в [руководстве по выпуску релизов](releasing.md).
