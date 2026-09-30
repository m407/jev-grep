# Поиск в логах

[English](../logs.md) | **Русский** · [Документация](README.md)

Это руководство описывает обычный текст, живые потоки и подготовленный JSONL.
Координаты вывода, опции и коды завершения см. в [CLI-справочнике](reference.md).

## Файлы и каталоги

```sh
# Один файл, вывод для человека.
jev-grep 'Certificate validation or trust chain failure' application.log

# Рекурсивно искать только в .log-файлах, повысить порог отбора.
jev-grep -g '*.log' --threshold 0.8 'Database connection failure' ./logs

# Машинный вывод.
jev-grep --format json 'Unresolved network failure' build.log

# До трёх выбранных фрагментов на каждый входной файл.
jev-grep -m 3 'Dependency resolution failure' first.log second.log
```

Примеры предполагают, что ключ уже передан; для Zen используйте
`fnox exec -- jev-grep ...` по [руководству по подключению](deployment.md).

Без путей `jev-grep` читает перенаправленный stdin, а при запуске из терминала обходит
текущий каталог. `-` явно обозначает stdin. При указанных путях stdin автоматически
не добавляется. Записи из разных входных файлов не объединяются.

Каталоги обходятся с учётом `.ignore` и применимых Git ignore-правил. Скрытые файлы
по умолчанию исключены, симлинки внутри каталогов не обходятся. Используйте
`--hidden` и `--no-ignore`, если нужно расширить область поиска. Включающий `-g`
не отменяет hidden/ignore-фильтры. Для каталога с бинарными файлами задайте glob:
некорректный UTF-8 и NUL приводят к ошибке.

## Вывод команды или живой поток

Примеры конвейеров ниже рассчитаны на Bash. Включите `pipefail`, чтобы ошибка
производителя данных также влияла на код завершения конвейера:

```bash
set -o pipefail

./gradlew --console=plain build 2>&1 |
  jev-grep 'Build failure caused by dependencies or certificate validation'

npm run build 2>&1 |
  jev-grep --format json 'Compilation or dependency error'

buildah bud . 2>&1 |
  jev-grep 'Image build failed because a command or download failed'

tail -f application.log |
  jev-grep 'Connection failure that was not resolved by a retry'
```

`2>&1` направляет stdout и stderr команды в один входной поток. У stdin координаты
начинаются с начала **полученного потока**. Например, `tail -f` не передаёт всю
историю файла: номера в результате не являются номерами строк полного
`application.log`. Чтобы получить координаты в сохранённом логе, передавайте
путь непосредственно: `jev-grep 'Build failure' build.log`.

## Как собирается фрагмент

Первая запись получает оценку `match`. Для текущего фрагмента и следующей записи
`jev-grep` отправляет **один HTTP-запрос с четырьмя вопросами**:

- `same_event` — продолжает ли запись то же конкретное событие;
- `useful_context` — существенно ли она объясняет, уточняет, разрешает или
  опровергает текущий фрагмент относительно критерия;
- `match_next` — подходит ли новая запись сама по себе;
- `match_extended` — подходит ли фрагмент вместе с новой записью.

Запись присоединяется, если `same_event` **или** `useful_context` достигает
`--join-threshold` (по умолчанию `0.5`). Общей темы самой по себе недостаточно.
При присоединении оценка заменяется на `match_extended`; при отказе текущий
фрагмент сохраняет прежнюю оценку, а новая запись начинает следующий.

Фрагмент завершается при смысловой границе, EOF входного источника или разбиении
по лимиту размера. После завершения он выводится, если его оценка не ниже
`--threshold` (по умолчанию `0.5`). Поэтому сообщение об успешном retry может
изменить отбор ранее найденной ошибки.

**Пауза в живом потоке не завершает фрагмент.** Даже подходящий текст ждёт следующую
границу или EOF; уже завершённые совпадения выводятся сразу со сбросом буфера stdout.
`--timeout` (по умолчанию 30 секунд) ограничивает одну полную модельную оценку,
включая отправку, чтение ответа, повторы и паузы. При временных HTTP/сетевых сбоях
выполняется максимум четыре попытки всего; паузы растут (200/400/800 мс) с jitter
и учитывают `Retry-After`. Ошибки авторизации, TLS и некорректные/слишком большие
ответы не повторяются. Повтор после сетевого сбоя может тарифицироваться повторно;
exactly-once inference не гарантируется.

## Подготовленные логи: JSONL и agrind

### Входной контракт log-jsonl

Каждая физическая строка должна быть JSON-объектом со строковым полем `text`:

```jsonl
{"text":"Connection failed: PKIX path building failed"}
{"text":"Retrying connection"}
{"text":"Connection established"}
```

```sh
jev-grep --input-format log-jsonl --format json \
  'Connection failure that was not resolved' prepared.jsonl
```

Поле `text` может содержать экранированные `\n`, то есть одну уже многострочную
запись. Соседние JSONL-записи всё равно проходят модельную проверку объединения.

**Используется только `text`.** Дополнительные `level`, `task`, `source`, `range`
и другие поля игнорируются, не передаются модели и не заменяют координаты.
Если эти сведения нужны для оценки, включите их в строковое поле `text`.
JSON-массив, пустая физическая строка, отсутствие `text` или его нестроковый тип
считаются ошибкой входа.

### agrind: переименовать сообщение в text

[Angle Grinder](https://github.com/rcoh/angle-grinder) устанавливается отдельно;
имя исполняемого файла — `agrind`. Примеры проверены с версией 0.19.6.

Для исходных JSONL-записей вида
`{"level":"error","message":"PKIX failure"}`:

```bash
set -o pipefail

agrind --output json '* | json | message as text | fields text' < app.jsonl |
  jev-grep --input-format log-jsonl --format json 'Certificate validation failure'
```

Здесь `message` должно быть строкой в каждой записи. Если для критерия важен
уровень сообщения и оба поля строковые, включите его в оцениваемый текст:

```bash
set -o pipefail

agrind --output json \
  '* | json | concat(level, ": ", message) as text | fields text' < app.jsonl |
  jev-grep --input-format log-jsonl 'Unresolved application error'
```

Для обычного текста возможна упаковка каждой строки:

```bash
set -o pipefail

agrind --output json '* | parse "*" as text noconvert' < build.log |
  jev-grep --input-format log-jsonl 'Dependency resolution failure'
```

`noconvert` оставляет числоподобные строки строками, как требует `jev-grep`.
При этом `agrind parse` удаляет крайние пробелы, включая отступы stack trace.
Чтобы сохранить исходный лог точно, используйте `jev-grep ... build.log` без преобразования.

Роль `agrind` — подготовка записей; **сборку многострочного фрагмента выполняет
модель внутри `jev-grep`**. Не добавляйте предварительный фильтр только по `error`, если
нужны продолжения stack trace или сообщения о восстановлении. Оператор `json`
в agrind отбрасывает невалидные JSON-строки; оператор `parse` без `nodrop` отбрасывает
несовпавшие строки. Такие потери upstream `jev-grep` обнаружить не сможет.

### Где искать совпадение после преобразования

Для `log-jsonl` диапазон относится к **фактически прочитанным JSONL-записям**,
а не к первоначальному логу. Текст результата — декодированные поля `text`,
соединённые одним дополнительным LF между записями (`text_basis: decoded-records`).

Если нужен файл, по которому можно открыть найденный диапазон, сохраните
подготовленный поток и ищите в нём:

```bash
agrind --output json '* | json | message as text | fields text' < app.jsonl > prepared.jsonl
jev-grep --input-format log-jsonl 'Certificate validation failure' prepared.jsonl
```

В результате будет путь `prepared.jsonl`. Путь `app.jsonl` и его исходные координаты
через произвольные поля JSON не переносятся.
