# Поиск кода через ast-grep

[English](../code-search.md) | **Русский** · [Документация](README.md)

[ast-grep](https://ast-grep.github.io/) определяет структурные границы, `jev-grep`
проверяет смысл. Используйте **`scan --json=stream`**: один объект на совпадение.
Примеры проверены с ast-grep 0.45.3. Настройте ключ по
[руководству по подключению](deployment.md); для Zen запускайте потребителя
через `fnox exec -- jev-grep ...`.

## Найти подходящие функции Rust

Команда работает из корня Rust-проекта с каталогом `src`:

```bash
set -o pipefail

ast-grep scan --json=stream --inline-rules '
id: rust-functions
language: Rust
severity: hint
rule:
  kind: function_item
' ./src |
  jev-grep --input-format ast-grep --max-line-bytes 262144 \
    'Function that validates input or rejects malformed data'
```

Для машинного вывода добавьте `--format json` к `jev-grep`.
`severity: hint` позволяет использовать правило для извлечения кандидатов;
error-level правило ast-grep может давать ненулевой exit code из-за самих совпадений.

Здесь лимит физической JSONL-строки повышен отдельно: ast-grep может повторять текст
в `text`, `lines`, `labels`, поэтому сериализованный объект больше тела функции.
Лимит самого тела `--max-fragment-bytes` остаётся по умолчанию 16384 байта.

## Проверить AST-кандидаты изменённых Rust-файлов

Из корня Git-репозитория следующий Bash-пример сравнивает tracked index и
рабочее дерево с `HEAD`, а затем сканирует текущую (рабочую) версию каждого
изменённого существующего `.rs` файла. Он намеренно выбирает **все** функции
этих файлов, а не только функции, пересекающие изменённые hunks. Удалённая
сторона diff не анализируется; такой поиск не обнаруживает удалённое поведение.
Untracked файлы не входят в список.

```bash
set -e
set -o pipefail

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

# Проверить Git отдельно: process substitution скрыла бы его exit status.
if ! git diff --name-only -z --diff-filter=ACMRTUXB HEAD -- >"$tmp"; then
  echo 'git diff failed' >&2
  exit 2
fi

files=()
while IFS= read -r -d '' path; do
  # Ограничить источники Rust; проверка существования исключает удаления.
  [[ $path == *.rs && -f $path ]] && files+=("./$path")
done < "$tmp"

if ((${#files[@]})); then
  ast-grep scan --json=stream --inline-rules '
id: rust-functions
language: Rust
severity: hint
rule:
  kind: function_item
' -- "${files[@]}" |
    jev-grep --input-format ast-grep --max-line-bytes 262144 \
      'Function that handles an error or validates input'
fi
```

NUL-разделение Git и Bash-массив сохраняют пути с пробелами и другими
специальными символами. Ветка пустого списка важна: запуск ast-grep без путей
может сканировать каталог по умолчанию. Ошибка Git останавливает пример до
сканирования и API-запроса. `jev-grep` получает только кандидатов ast-grep и
не читает историю Git и не разбирает diff.

## Python-код через stdin

```bash
set -o pipefail

cat service.py |
  ast-grep scan --stdin --json=stream --inline-rules '
id: python-functions
language: Python
severity: hint
rule:
  kind: function_definition
' |
  jev-grep --input-format ast-grep --max-line-bytes 262144 --format json \
    'Function that retries a failed network request'
```

В этой цепочке источником кода будет `stdin`, а не `service.py`: ast-grep получает
код без пути. Чтобы сохранить имя файла, передайте `service.py` непосредственно
в ast-grep вместо `cat ... | ... --stdin`.

Сам ast-grep читает код из stdin до EOF. Полученные готовые JSONL-кандидаты `jev-grep`
оценивает и выводит по одному, без ожидания EOF всего потока кандидатов.

## Собственные правила и сохранённые кандидаты

Типы AST-узлов зависят от языка. Для своего набора функций, методов или объявлений
задайте правило ast-grep, например в `rule.yml`, с подходящими `language`, `kind`
и `severity: hint`:

```bash
set -o pipefail

ast-grep scan --json=stream --rule rule.yml ./src |
  jev-grep --input-format ast-grep --max-line-bytes 262144 --format json \
    'Code that handles authentication failures'
```

Либо сохраните результат ast-grep и повторяйте семантические запросы:

```sh
jev-grep --input-format ast-grep --max-line-bytes 262144 \
  'Code that handles authentication failures' candidates.jsonl
```

## Входной контракт кодового режима

- Каждый кандидат получает один вопрос `match`. Соседние совпадения не группируются;
  `--join-threshold` на этот режим не влияет.
- Оценивается именно `text`, не контекст из `lines`. Вложенные, пересекающиеся
  и повторные узлы остаются отдельными кандидатами. Их выбор задаётся upstream-правилом.
- Принимаются поля `text`, `file`, `range.byteOffset.start/end`,
  `range.start.line/column`, `range.end.line/column`; `language` необязателен.
  `jev-grep` проверяет согласованность текста и диапазонов, но не открывает исходный файл
  заново и не проверяет, изменился ли он после запуска ast-grep.
- Обычный JSON-массив от `--json`/`--json=compact` и объекты `outline` не подходят.
- `-g`, `--hidden`, `--no-ignore` у `jev-grep` применяются к входным файлам JSONL,
  а не к `file` внутри кандидатов. Отбор исходных файлов кода настройте в ast-grep.

Координаты вывода, подсчёт по источникам и лимиты см. в
[CLI-справочнике](reference.md).
