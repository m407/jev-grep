# Автоматические релизы и Conventional Commits

[English](../releasing.md) | **Русский** · [Документация](README.md)

Каждый push в `main` запускает [Release](../../.github/workflows/release.yml).
`semantic-release` анализирует коммиты после последнего тега `v*`, определяет
новую версию и генерирует release notes. Ручное повышение версии не требуется.

## Автоматическое слияние Dependabot

Pull requests проходят те же проверки release tooling, форматирования, Clippy,
Rust и Python, что и релизы, а также проверку Conventional Commits. После успеха
всего PR workflow [Dependabot auto-merge](../../.github/workflows/dependabot-auto-merge.yml)
выполняет squash-слияние PR Dependabot в `main`, включая major-обновления и
исправления безопасности. Перед слиянием проверяются автор, исходный репозиторий
и совпадение head SHA с проверенным коммитом. Ошибки проверок и конфликты
блокируют автоматическое слияние.

Workflow слияния использует `workflow_run` и встроенный `GITHUB_TOKEN`, не
выгружая код PR и не требуя персонального токена. После слияния он явно запускает
Release на `main`: слияния через `GITHUB_TOKEN` не вызывают push workflows.
Успешный CI workflow служит условием слияния; включённая защита ветки для этого
не требуется.

## Формат коммитов

```text
fix: correct timeout handling
feat(cli): add a new output format
docs: clarify installation
feat(api)!: change the response format
```

| Изменение | Версия |
| --- | --- |
| `fix:`, `perf:` | patch |
| `feat:` | minor |
| `!` после типа/scope или footer `BREAKING CHANGE: ...` | major |
| `docs:`, `chore:`, `test:`, `ci:`, `build:`, `refactor:`, `style:` без breaking change | Без релиза |

Выбирается наибольшее повышение среди всех новых коммитов. Breaking change
означает major и для версий `0.x`. Заголовки PR и все коммиты PR проверяются
`commitlint`; при squash merge сохраните Conventional Commit в итоговом сообщении.
Коммиты каждого push в `main` также проверяются перед выпуском.

Локальная проверка (Node.js 24.10+):

```sh
npm ci
printf '%s\n' 'feat(cli): add a new output format' | npm run commitlint
npm run commitlint -- --last --verbose
npm run test:release
```

## Что делает workflow

1. Проверяет сообщения коммитов, release tooling, форматирование Rust, Clippy,
   Rust-тесты и Python acceptance-тесты с локальным mock API.
2. Вычисляет версию через dry-run semantic-release. Если релиз не нужен,
   сборка архивов и публикация пропускаются.
3. Обновляет версии пакета в `Cargo.toml` и `Cargo.lock` в каждой сборке,
   сохраняя версии зависимостей. Собирает `cargo build --release --locked`:

   | Платформа | Target | Архив |
   | --- | --- | --- |
   | Linux x86-64 (glibc, Ubuntu 24.04) | `x86_64-unknown-linux-gnu` | `.tar.gz` |
   | macOS Apple Silicon | `aarch64-apple-darwin` | `.tar.gz` |
   | Windows x86-64 | `x86_64-pc-windows-msvc` | `.zip` |

4. Проверяет `--help` и точное совпадение `--version` с рассчитанной версией.
5. Только после всех успешных сборок формирует `SHA256SUMS` и повторно проверяет
   рассчитанную версию перед публикацией.
6. Коммитит `Cargo.toml` и `Cargo.lock` в `main` сообщением
   `chore(release): X.Y.Z [skip ci]`, создаёт тег `vX.Y.Z` и GitHub Release
   с архивами и release notes. Публикацией управляет semantic-release.

Архивы содержат бинарник и README. Проверка скачанных файлов на Linux:

```sh
sha256sum --ignore-missing -c SHA256SUMS
```

Сборка и публикация выполняются одним workflow: отдельный запуск по тегу
не требуется. Workflow использует встроенный `GITHUB_TOKEN`; доступ к Zen
и ключ модели не нужны. npm-пакет и пакет на crates.io не публикуются.

## Первый релиз и настройки GitHub

При отсутствии предыдущих release-тегов semantic-release начинает с **1.0.0**,
независимо от текущей версии в `Cargo.toml`. Если нужно продолжить линию `0.1.x`,
сначала обозначьте уже выпущенный коммит тегом `v0.1.0` и отправьте этот тег
в GitHub. Следующий релиз будет рассчитан по коммитам после него.

GitHub Actions должны иметь возможность записывать contents. Если `main`
защищена правилами, разрешите release-боту push version-коммита; иначе публикация
остановится. В репозитории рекомендуется сделать `Conventional Commits / commitlint`
обязательной проверкой PR и настроить squash merge с заголовком PR в качестве
сообщения коммита. Эти настройки выполняются в GitHub, а не YAML-файлами.

Запуски релиза сериализованы. При продвижении `main` во время сборки устаревший
запуск может не опубликовать релиз; следующий запуск обработает новые коммиты.
Для повтора доступен ручной запуск Release на ветке `main`.

Если ошибка случилась после отправки version-коммита/тега, проверьте существующий
тег и GitHub Release перед повтором: semantic-release не перепубликует автоматически
уже помеченную версию. Сохранённые workflow artifacts можно использовать для
восстановления недостающих release assets.
