# Подключение Zen и локальные ключи

[English](../deployment.md) | **Русский** · [Документация](README.md)

`jev-grep` — клиент OpenCode Zen; fnox передаёт ему ключ API при запуске процесса.
По умолчанию приложение использует endpoint Zen System One
`https://opencode.ai/zen/v1/systemone` и модель `jev-1.13`. Модель передаётся
явно. `fnox` — средство запуска, а не зависимость самого бинарника.

## Требования

- Установите бинарник `jev-grep` по [инструкции по установке](installation.md).
- Установите fnox и добавьте его в `PATH` для запусков с ключом.
- На Linux нужен доступный разблокированный Secret Service (обычно через
  хранилище ключей рабочего стола, например GNOME Keyring, или другой совместимый
  демон). В headless/SSH-сессии сервис может быть заблокирован; разблокируйте
  его или настройте доступ локально до запуска fnox. Перехода к хранению ключа
  открытым текстом намеренно нет.

Проектный `fnox.toml` ссылается на запись в keychain; самого ключа API в нём нет.
`root = true` делает конфигурацию проекта корневой для fnox, прекращая поиск
в родительских каталогах. Глобальную конфигурацию fnox это не отключает.

## TLS-сертификаты и системное доверие

Проверка HTTPS использует объединение доверенных корней операционной системы
и встроенных корней WebPKI бинарника. Системные корни расширяют доверие, а не
заменяют встроенный набор; установка частного CA не отключает проверку цепочки,
имени узла или срока действия сертификата. Хранилище доверия CA не связано с
записью системного keychain, которую fnox использует для `JEV_GREP_API_KEY`:
установка CA не настраивает ключ API, а сохранение ключа не делает CA сервера
доверенным.

Загрузчик сертификатов учитывает заданные `SSL_CERT_FILE` и `SSL_CERT_DIR`:
они переопределяют системный источник сертификатов. Встроенные корни WebPKI
при этом остаются доступны. Для проверки настоящего хранилища ОС удалите обе
переменные окружения. Сертификаты читаются при создании HTTP-клиента, поэтому
установите или удалите CA до запуска `jev-grep` и запускайте новый процесс после
каждого изменения хранилища.

### Проверка временного CA в одноразовом окружении

Проводите проверку только в одноразовом окружении, не на рабочей машине.
Три обязательные release-target: **Linux GNU** (`x86_64-unknown-linux-gnu`),
**macOS ARM64** (`aarch64-apple-darwin`) и **Windows MSVC**
(`x86_64-pc-windows-msvc`), согласно [матрице сборки релизов](../../.github/workflows/release.yml).
Установите соответствующий бинарник `jev-grep`, Python 3 и OpenSSL. HTTPS-
фикстура использует стандартную библиотеку Python; Python, OpenSSL и CLI должны
работать в одном одноразовом окружении. В Linux нужен Bash/coreutils; в macOS —
zsh или Bash и штатная утилита `security`; в Windows — 64-разрядный PowerShell 7,
OpenSSL (например, из Git for Windows) и штатные командлеты
`Import-Certificate`/хранилища сертификатов. Блок OpenSSL ниже использует POSIX
shell (Bash/zsh); в Windows запускайте его в Git Bash, а команды хранилища — в
PowerShell. Для запуска CLI права администратора не нужны, но установка CA в
машинное корневое хранилище требует повышения прав.

Используйте этот исполняемый harness для проверки системного доверия с реальной
локальной HTTPS-фикстурой. Он импортирует генератор сертификатов и серверные
вспомогательные методы из [`tests/test_cli.py`](../../tests/test_cli.py), но
**не** использует переопределения `SSL_CERT_FILE`/`SSL_CERT_DIR` тех тестов.
Сохраните код ниже как `/tmp/jev-native-ca-check.py` (в Windows —
`%TEMP%\jev-native-ca-check.py`) и запустите `PYTHONPATH=.
python3 /tmp/jev-native-ca-check.py` (Windows PowerShell:
`$env:PYTHONPATH=(Get-Location).Path; py -3 "$env:TEMP\jev-native-ca-check.py"`).
Запускайте из корня репозитория после `cargo build --locked`. Для использования
бинарника конкретного target/release задайте `JEV_GREP_TEST_BIN` с его путём
перед запуском harness. Harness выводит точный путь к CA и HTTPS URL, нужный
для команд установки ОС ниже, затем ждёт установки/удаления сертификата в
одноразовом системном хранилище. Не завершайте процесс до конца всех этапов.

```python
import os
import subprocess
import sys
import tempfile
from pathlib import Path

from tests.test_cli import BIN, CliTests, clean_test_env

binary = Path(os.environ.get('JEV_GREP_TEST_BIN', BIN))
if os.name == 'nt' and binary.suffix.lower() != '.exe':
    binary = binary.with_suffix('.exe')

fixture = CliTests(methodName='test_tls_trusted_ca_allows_single_and_group_evaluation')
fixture.setUp()
folder = tempfile.TemporaryDirectory(prefix='jev-native-ca-')
ca, cert, key, _, _ = fixture.make_test_certificates(folder.name)
try:
    with fixture.run_tls_server(cert, key) as server:
        url = f'https://localhost:{server.server_port}/v1/systemone'
        env = clean_test_env()
        for name in ('SSL_CERT_FILE', 'SSL_CERT_DIR'):
            env.pop(name, None)

        def check(stage, expected):
            for label, args, data, expected_requests in (
                ('single', ['criterion'], 'PKIX\n', 1),
                ('grouped', ['--format', 'json', 'criterion'],
                 'PKIX first\njoin continuation\n', 2),
            ):
                fixture.requests.clear()
                server.connection_count = 0
                result = subprocess.run(
                    [str(binary), '--api-url', url, *args], input=data, text=True,
                    capture_output=True, timeout=10, env=env)
                assert result.returncode == expected, (
                    stage, label, result.returncode, result.stderr)
                if expected == 0:
                    assert len(fixture.requests) == expected_requests, (
                        stage, label, len(fixture.requests))
                    assert server.connection_count == expected_requests
                else:
                    assert not fixture.requests, (stage, label, fixture.requests)
                    assert server.connection_count == 1, (
                        stage, label, server.connection_count)
                    assert not result.stdout, (stage, label, result.stdout)
                print(f'{stage}: {label}: exit={result.returncode}, '
                      f'connections={server.connection_count}, '
                      f'HTTP requests={len(fixture.requests)}')

        print(f'Temporary CA certificate (install this file): {Path(ca).resolve()}')
        print(f'Local fixture URL: {url}')
        check('before installation', 2)
        input('Install this CA in the disposable OS store; then press Enter: ')
        check('after installation', 0)
        input('Remove this exact CA from the OS store; then press Enter: ')
        check('after removal', 2)
finally:
    folder.cleanup()
    fixture.doCleanups()
```

**Одиночная** команда передаёт `PKIX\n`; **групповая** —
`PKIX first\njoin continuation\n`. На каждом этапе harness запускает новый
CLI-процесс для каждого пути без CA-переопределений. До установки и после
удаления он проверяет код 2, одно TLS-соединение, ноль HTTP-запросов и пустой
stdout; после установки — код 0, обработку корректного ответа фикстуры, один
запрос для одиночного и два для группового пути. При нарушении ожиданий harness
завершается с ошибкой. Фикстура использует временный loopback-порт и только
синтетический вход; Zen endpoint и credentials не используются.

Установите только CA, путь к которому напечатал harness, в одноразовое
окружение, затем удалите именно этот сертификат по fingerprint/thumbprint и
обновите bundle Linux, если это применимо:

| Окружение | Установка CA | Удаление только этого CA |
| --- | --- | --- |
| Linux GNU (Debian/Ubuntu) | `sudo install -m 0644 '<путь-к-CA-из-вывода>' /usr/local/share/ca-certificates/jev-grep-test.crt && sudo update-ca-certificates` | `sudo rm /usr/local/share/ca-certificates/jev-grep-test.crt && sudo update-ca-certificates` |
| macOS ARM64 | `sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain '<путь-к-CA-из-вывода>'` | `sudo security delete-certificate -Z "$(openssl x509 -in '<путь-к-CA-из-вывода>' -noout -fingerprint -sha1 | cut -d= -f2 | tr -d :)" /Library/Keychains/System.keychain` |
| Windows MSVC (PowerShell 7 с повышенными правами) | `Import-Certificate -FilePath '<путь-к-CA-из-вывода>' -CertStoreLocation Cert:\LocalMachine\Root` | `$ca = Get-PfxCertificate -FilePath '<путь-к-CA-из-вывода>'; Get-ChildItem Cert:\LocalMachine\Root | Where-Object Thumbprint -eq $ca.Thumbprint | Remove-Item` |

После каждого изменения хранилища запускайте новые процессы CLI. Перед
удалением временных ключей/сертификатов и уничтожением окружения убедитесь,
что CA отсутствует в изолированном хранилище. Не удаляйте и не изменяйте другие
сертификаты. См. [инструкцию по установке](installation.md) и [английскую
версию руководства по развёртыванию](../deployment.md).

## Сохранение ключа

Из корня репозитория выполните команду, не передавая значение:

```sh
fnox set JEV_GREP_API_KEY --provider zen-keychain
```

fnox интерактивно запросит ключ со скрытым вводом. Не передавайте ключ аргументом
команды, не сохраняйте его в истории shell и не коммитьте. Провайдер системного
keychain сохраняет секрет под сервисом `jev-grep`. Git игнорирует
`fnox.local.toml` для любых локальных переопределений fnox.

Запускайте клиент через fnox, чтобы секрет был получен до запуска дочернего процесса:

```sh
fnox exec -- jev-grep --json 'Certificate validation failure' sample.log
```

Если keychain недоступен или запись отсутствует, fnox должен завершиться с ошибкой
до запуска `jev-grep`. Не заменяйте это пустым ключом или текстовым файлом.
Прямой запуск без fnox возможен для явно настроенных mock/собственных endpoint;
он не обеспечивает обычную настройку ключа Zen.

## Ручные переопределения

Клиент принимает флаги и соответствующие переменные окружения. Флаги имеют
приоритет над переменными, а переменные — над значениями по умолчанию.
Используйте только новые имена `JEV_GREP_*`:

```sh
JEV_GREP_API_URL='https://opencode.ai/zen/v1/systemone' \
JEV_GREP_MODEL='jev-1.13-free' \
fnox exec -- jev-grep 'Certificate validation failure' sample.log
```

`--api-url`, `--model` и `--api-key` переопределяют соответствующие настройки
окружения. Для ключа API предпочитайте fnox; не помещайте ключ в историю shell
или аргументы процесса. Вручную выбранный endpoint должен быть полным адресом
System One. Автоматического переключения endpoint/модели нет.

## Данные, стоимость и контекст

`--timeout` задаёт общий бюджет одной модельной оценки, по умолчанию 30 секунд,
включая повторы, чтение тела ответа и ожидание. При временных HTTP/сетевых сбоях
выполняется максимум четыре попытки всего с экспоненциальной задержкой, jitter
и учётом `Retry-After`; ошибки авторизации, проверки TLS, некорректные
и слишком большие ответы не повторяются. Повтор после транспортного сбоя
может привести к повторной оплате inference; exactly-once inference не гарантируется.

Запросы отправляют входной текст выбранному endpoint. Используйте только данные,
которые вам разрешено передавать; для тестирования предпочтительны синтетические
примеры. Доступность и цены моделей Zen могут меняться; перед подключением проверьте
[актуальную документацию Zen Jev](https://opencode.ai/docs/zen/#jev), а не полагайтесь
на скопированную цену. При проверке 2026-09-25 страница указывала для `jev-1.13`
$0.042 за миллион входных токенов и бесплатные выходные токены; цена кешированных
токенов не была указана. Цены и доступность могут измениться. Выбранная модель —
`jev-1.13`; бесплатный ID `jev-1.13-free` используется только при явном выборе,
как в примере выше.

Согласно документации моделей Jev на 2026-09-25, прямой TypeSafe API описывает
контекст 64k для state и всех вопросов вместе и 32k для state и самого длинного
вопроса одновременно ([документация моделей](https://docs.typesafe.ai/models)).
Это описания внешней модели/API, а не проверенные лимиты шлюза Zen.
Из них нельзя получить число токенов по количеству входных байтов.
Лимиты фрагментов клиента заданы в байтах и независимы от токенового контекста
модели. Измеряйте на разрешённых синтетических входах; один длинный вход
сам по себе не доказывает лимит контекста.

Будущая методика оценки отложена в [backlog проекта](BACKLOG.md).
Синтетический seed не является результатом оценки качества Jev.

## Минимальная живая интеграционная проверка

Проверка выполнена **2026-09-28** release-бинарником, собранным через
`cargo build --locked --release`. Запрошенная модель: `jev-1.13`;
запрошенный endpoint: `https://opencode.ai/zen/v1/systemone`. Оба запуска
использовали `fnox exec` с настроенной записью keychain; значение ключа
не выводилось и не сохранялось. Синтетические входы временно хранились
в `/tmp/opencode/jev-smoke-20260928`.

Воспроизведение из корня репозитория после настройки fnox, описанной выше:

```sh
cargo build --locked --release
mkdir -p /tmp/opencode/jev-smoke-20260928
printf '%s\n' '{"text":"fn validate_certificate() { true }","file":"synthetic.rs","language":"Rust","range":{"byteOffset":{"start":0,"end":34},"start":{"line":0,"column":0},"end":{"line":0,"column":34}}}' > /tmp/opencode/jev-smoke-20260928/ast.jsonl
printf '%s\n' 'certificate validation failed for service alpha' 'certificate validation succeeded for service beta' > /tmp/opencode/jev-smoke-20260928/log.txt
fnox exec -- target/release/jev-grep --input-format ast-grep --json \
  'Find certificate validation code that may explain a validation failure' \
  /tmp/opencode/jev-smoke-20260928/ast.jsonl
fnox exec -- target/release/jev-grep --json \
  'Find log records relevant to a certificate validation failure' \
  /tmp/opencode/jev-smoke-20260928/log.txt
```

Результаты: один валидный синтетический AST-кандидат обработан с кодом **1**
(нет выбранного фрагмента), без stderr и ошибок API/входа. Двухстрочный текстовый
вход обработан с кодом **0** с выводом выбранной первой записи; без `-q` или `-m`
обработка продолжилась до второй записи и групповой оценки перед EOF.
Ни один результат не устанавливает качество модели. Это только свидетельство
работы протокола/интеграции; benchmark, утверждения о качестве, статистика
задержек или usage не собирались. Каждый сценарий запускался один раз.
