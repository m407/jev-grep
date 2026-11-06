# Автоматические релизы и Conventional Commits

[English](../releasing.md) | **Русский** · [Документация](README.md)

Каждый push в `main` запускает [Release](../../.github/workflows/release.yml).
Закреплённый npm-пакет `release-please` **17.11.2** создаёт или обновляет release
PR с новой версией и changelog на основе Conventional Commits. После успешного CI
для точного head этот PR автоматически сливается через squash; публикация затем
зависит от полной проверки слитого снимка и сборок для трёх платформ.
Ручное повышение версии или одобрение
сгенерированного release PR не требуется после настройки релизного GitHub App.

[release-please-config.json](../../release-please-config.json) настраивает корневой
Rust-пакет; [.release-please-manifest.json](../../.release-please-manifest.json)
хранит его выпущенную версию. npm-зависимость закреплена в
[package-lock.json](../../package-lock.json); GitHub Action release-please не
используется. Release-please управляет release PR, версиями и changelog;
за публикацию отвечает [инструментарий репозитория](../../scripts/release.mjs).

## Защищённое автоматическое слияние PR ботов

Pull requests и релизы выполняют общий набор `mise run check` из
[руководства для разработчиков](development.md) и проверку Conventional Commits.
После успеха всего PR workflow
[Bot PR auto-merge](../../.github/workflows/dependabot-auto-merge.yml)
выполняет squash-слияние PR Dependabot в `main`, включая major-обновления и
исправления безопасности. Перед слиянием проверяются автор, исходный репозиторий
и совпадение head SHA с проверенным коммитом. Ошибки проверок и конфликты
блокируют автоматическое слияние.

Этот же защищённый workflow обрабатывает сгенерированные release PR. Он проверяет
идентичность workflow Release и успех задач валидации, автора
`<RELEASE_APP_SLUG>[bot]` из конфигурации, тот же репозиторий, базовую ветку `main`, исходную ветку
`release-please--branches--main`, release labels, допустимые файлы и точный
проверенный head SHA. Допустимы только `Cargo.toml`, `Cargo.lock`,
`.release-please-manifest.json` и `CHANGELOG.md`. Обычные PR участников, включая
PR миграции #5, не подлежат этому автоматическому слиянию.

[Конфигурация Dependabot](../../.github/dependabot.yml) охватывает Cargo, npm и
GitHub Actions с периодом ожидания (cooldown) для обновлений версий. Это не
заменяет проверку успешного workflow и точного head SHA перед слиянием.
Плановое сканирование OSV описано в
[руководстве для разработчиков](development.md#безопасность-и-найденные-проблемы).

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

Выбирается наибольшее повышение среди всех новых коммитов. Заголовки PR и все
коммиты PR проверяются `commitlint`; при squash merge сохраните Conventional
Commit в итоговом сообщении.
Коммиты каждого push в `main` также проверяются перед выпуском.

После [настройки окружения](development.md#настройка) установленный commit-msg hook
проверяет сообщения локальных коммитов. Ручная проверка и полный набор:

```sh
mise run check:commits -- --last --verbose
mise run check
```

`commitlint` — псевдоним задачи `check:commits`.

## Что делает workflow

1. Проверяет сообщения коммитов и выполняет `mise run check`, включая существующие
   JavaScript-тесты автоматизации релизов, Rust- и Python-тесты и общие проверки.
2. На `main` подготовка сначала ищет слитый release PR с меткой
   `autorelease: pending` и возобновляет его публикацию. Иначе release-please
   создаёт или обновляет release PR, генерируя `CHANGELOG.md` и синхронно обновляя
   `Cargo.toml`, `Cargo.lock` и manifest. Если слитого релиза, ожидающего
   публикации, нет, сборка архивов и публикация пропускаются.
3. Создаёт/обновляет PR короткоживущим токеном GitHub App, поэтому обычный PR CI
   запускается без подтверждения. Также явно запускает валидацию Release для
   точного head, включая неизменившиеся PR, для поддержки повторов. Обе проверки
   могут выполняться; сериализованный workflow слияния повторно проверяет PR.
   После успешной проверки защищённый workflow ботов выполняет
   squash-слияние точного проверенного head и явно запускает Release на `main`.
4. Подготовка возвращает неизменяемый SHA merge-коммита release PR и версию.
   Отдельная задача `release-check` выполняет полный `mise run check` на этом SHA
   параллельно с матрицей сборок.
   Каждая платформа выгружает этот SHA, проверяет объявленную версию и собирает
   через `cargo build --release --locked`:

   | Платформа | Target | Архив |
   | --- | --- | --- |
   | Linux x86-64 (glibc, Ubuntu 24.04) | `x86_64-unknown-linux-gnu` | `.tar.gz` |
   | macOS Apple Silicon | `aarch64-apple-darwin` | `.tar.gz` |
   | Windows x86-64 | `x86_64-pc-windows-msvc` | `.zip` |

5. Проверяет `--help` и точное совпадение `--version` с версией релиза.
6. Только после успеха `release-check` и всех трёх сборок формирует `SHA256SUMS`.
   Публикатор проверяет checkout, слитый PR, версию, наличие ровно трёх архивов
   и их контрольные суммы.
   Создаёт тег `vX.Y.Z` на неизменяемом merge SHA или проверяет, что существующий
   тег указывает туда, затем создаёт или возобновляет черновик GitHub Release.
7. Загружает полный набор assets и скачивает его обратно для проверки имён,
   размеров и контрольных сумм. Только затем публикует черновик, заменяет
   `autorelease: pending` на `autorelease: tagged` и запускает Release на `main`
   для обработки дальнейших изменений.

Архивы содержат бинарник и README. Проверка скачанных файлов на Linux:

```sh
sha256sum --ignore-missing -c SHA256SUMS
```

Сборка и публикация выполняются одним workflow после слияния release PR:
отдельный запуск по тегу не требуется. Создание PR использует installation token
GitHub App; слияние, публикация и явный dispatch используют встроенный
`GITHUB_TOKEN`. Персональный токен, доступ к Zen и ключ модели не нужны.
npm-пакет и пакет на crates.io не публикуются.

## Базовая версия и настройки GitHub

Миграция начинается с существующего релиза корневого пакета **1.1.0**. Manifest
хранит эту базовую версию, а `bootstrap-sha` задаёт начальную границу поиска
коммитов: `34610d0bed5d374d0ccd0c1304f363614668cb30`. Это не принудительная
следующая версия: повышение определяется Conventional Commits после базового
релиза. Далее используется история слитых release PR release-please.

### Релизный GitHub App (обязательно)

Создайте отдельный GitHub App в **Settings → Developer settings → GitHub Apps**.
Отключите webhook и выдайте права репозитория **Contents: Read and write** и
**Pull requests: Read and write** (чтение Metadata предоставляется автоматически).
Установите App только в этот репозиторий. Сгенерируйте приватный ключ и настройте
**Settings → Secrets and variables → Actions** репозитория:

| Тип | Имя | Значение |
| --- | --- | --- |
| Variable | `RELEASE_APP_CLIENT_ID` | Client ID из настроек App |
| Variable | `RELEASE_APP_SLUG` | Точный slug из `github.com/apps/<slug>`, без `[bot]` |
| Secret | `RELEASE_APP_PRIVATE_KEY` | Полное содержимое сгенерированного PEM-ключа |

Закреплённый по SHA action `actions/create-github-app-token` создаёт токен только
для текущего репозитория с этими двумя правами записи и отзывает его по завершении
задачи. Ключ получает только подготовка на `main`. PR CI и проверка сборок без
публикации не требуют credentials App. Подготовка проверяет совпадение slug App
токена с конфигурацией; валидация релиза, слияние и публикация допускают только
бота этого App. Без credentials подготовка завершается ошибкой; возврата к
созданию PR через `GITHUB_TOKEN` нет.

Теперь GitHub создаёт PR-запуски с обязательным подтверждением при создании или
обновлении PR через `GITHUB_TOKEN`. Настройка **Allow GitHub Actions to create and
approve pull requests** это требование не отменяет. Installation token App —
[документированный способ](https://docs.github.com/en/actions/concepts/security/github_token)
автоматического запуска. Права workflows по умолчанию остаются read-only;
отдельные задачи получают права contents, pull-request и Actions для оркестрации.
Правила ветки,
если они есть, должны разрешать squash-слияние ботом после CI. Рекомендуется
использовать заголовок PR в формате Conventional Commit как сообщение squash
merge. Эти настройки репозитория выполняются в GitHub, а не YAML-файлами.

### Миграция и проверка внедрения

Обновление PR токеном App не меняет его автора. Перед первой подготовкой через
App закройте неслитый release PR от `github-actions[bot]`, если он есть, и удалите
его ветку `release-please--branches--main`. После развёртывания конфигурации и
установки App запустите Release на `main`. Release-please пересоздаст ожидающий
релиз из коммитов с правильным автором; старый PR вручную не сливайте.
Уже слитый legacy pending-релиз нужно завершить старым публикатором до миграции:
новый публикатор отклонит его автора.

В этом репозитории legacy PR #15 уже слит, а `v1.2.0` опубликован до миграции
на App. Пересоздавать его не нужно; следующий релиз использует новый PR.
App `m407-jev-grep-release` установлен, variables и secret репозитория настроены.
Локальный ключ сохранён в системном keychain через профиль fnox `release`
(`RELEASE_APP_PRIVATE_KEY`); скачанный PEM удалён. Для команд, которым нужен ключ
без загрузки credentials Zen, используйте
`fnox --profile release --no-defaults exec -- <command>`.

Проверьте автора нового PR, автоматически запущенный Release по `pull_request`,
успех Bot PR auto-merge и последующую публикацию трёх архивов и `SHA256SUMS`.
Локальные тесты не проверяют установку App и эту сквозную цепочку в GitHub.
Если CI успешен, а workflow слияния не появился, повторите обычный PR workflow
и проверьте последующий запуск, прежде чем считать внедрение завершённым.

## Проверка сборок без публикации

Чтобы проверить выбранную ветку, включая ветку PR миграции #5, замените
`<branch>` её именем и выполните:

```sh
gh workflow run release.yml --ref <branch> -f verify_builds=true
```

Оставьте `release_pr` и `release_sha` пустыми. Этот режим запускает `check` и
матрицу сборок Linux/macOS/Windows на точном SHA и версии из контекста, затем
загружает архивы как workflow artifacts. Задачи `commitlint`, `prepare`,
`release-check` и `publish` пропускаются: PR не создаются, слияния не выполняются,
релизы не публикуются, даже если выбрана ветка `main`.

## Повтор публикации

Запуски Release сериализованы для каждого ref. Для повтора вручную запустите
Release на `main` с `verify_builds` равным false и пустыми параметрами release PR.
Подготовка возобновляет самый старый слитый pending-релиз до создания нового
release PR. Сборки остаются привязаны к его merge SHA даже при продвижении `main`.

Повтор заменяет весь набор assets черновика, включая `SHA256SUMS`, поэтому
результаты разных попыток сборки не смешиваются. Уже опубликованный релиз
проверяется перед восстановлением меток PR. Существующий тег, указывающий на
другой коммит, вызывает ошибку; автоматизация никогда не перемещает такой тег.
Частично загруженные assets остаются в черновике, а не становятся публичным
неполным релизом.
