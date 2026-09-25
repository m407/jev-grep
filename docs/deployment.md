# Развёртывание Laya API для lg

Исследование upstream на 2026-09-23. Источники:

- [README: установка, hosted API и self-hosting](https://github.com/NandhaKishorM/laya/blob/main/README.md)
- [Реальный HTTP-контракт и настройки: laya/serve.py](https://github.com/NandhaKishorM/laya/blob/main/laya/serve.py)
- [Docker quickstart и требования](https://github.com/NandhaKishorM/laya/blob/main/docs/docker.md)

Эти ссылки указывают на изменяемый `main`. README исследованной версии описывает 0.3.7.
Context7 и Docker-документация местами отстают: Docker-страница ещё называет HTTP serving
ожидаемым, хотя `serve.py` и команда `laya-serve` уже описаны в README.
Поэтому SDK Docker quickstart нельзя выдавать за готовый HTTP deployment.

## Архитектура и требования

`lg` (Rust, без Python и весов) → HTTP POST → FastAPI/Uvicorn `laya-serve` → Router → PyTorch → checkpoint.

| Компонент | Подтверждённые сведения / ограничения |
| --- | --- |
| Python сервера | 3.10+ согласно README; для новой установки разумно 3.11/3.12 |
| CPU | Поддерживается, GPU не обязателен |
| RAM/диск | Upstream Docker CPU quickstart просит **8 GB RAM и 10 GB свободного диска**. Это рекомендация quickstart, не доказанный минимум для всех трёх моделей одновременно |
| GPU | NVIDIA/CUDA опционально; совместимые драйвер/PyTorch, для Docker — NVIDIA Container Toolkit. Docker GPU quickstart использует CUDA 12.8 wheels |
| VRAM | Фиксированный минимум upstream не указан: зависит от checkpoint, длины и batch. Нельзя обещать запуск на определённом объёме без замера |
| Веса | English / typed-decisions: ModernBERT-large, 421M параметров; multilingual: mmBERT-base, 322M |
| Память весов | Только оценка: FP32 веса 421M ≈ 1.68 GB, 322M ≈ 1.29 GB; все три ≈ 4.66 GB. Это **не** общий RSS/VRAM: нужны активации, runtime, буферы и запас |
| Первая загрузка | Доступ к Hugging Face для публичных checkpoint; сохраняйте HF-кеш между запусками |
| Производительность | Upstream сообщает ~33–40 ms/вопрос на T4 и ~193–464 ms на CPU для приведённых сценариев. Это не измерения `lg` и не SLA |
| Лицензия | Репозиторий заявляет Apache-2.0; перед распространением весов отдельно проверьте лицензию выбранного checkpoint |

Умножать batch throughput вопросов к одному state на число строк `lg` некорректно.
Каждый HTTP-запрос оценивает один state; `questions` — именованный объект независимых
решений над общими данными state, а не массив документов. `lg` использует одиночный
`match` для первой записи и один групповой запрос с четырьмя вопросами при проверке
следующей записи.

## Локальный CPU-сервер

Команды для Linux/macOS; выполнять отдельно от сборки Rust:

```sh
python3 -m venv .venv
# CPU-only wheels, чтобы не устанавливать CUDA runtime без необходимости:
.venv/bin/python -m pip install torch --index-url https://download.pytorch.org/whl/cpu
.venv/bin/python -m pip install 'laya[serve]==0.3.7'
.venv/bin/python -I -c 'import laya; print(laya.__version__)'

# Только English preload для начала. Явно ограничиваем доступ localhost.
LAYA_HOST=localhost LAYA_PORT=8000 LAYA_DEVICE=cpu \
LAYA_PRELOAD=1 LAYA_MODELS=english LAYA_THREADS=4 \
  .venv/bin/laya-serve
```

`LAYA_THREADS` не должен превышать доступные физические ядра. `LAYA_MODELS` управляет
**preload**, а не запретом других моделей: auto-router может загрузить multilingual позже.
Чтобы фиксировать checkpoint для запросов клиента, передавайте `lg --model english`.
Для смешанных языков рассмотрите preload `english,multilingual` и измерьте RSS.

Если pinned release недоступен или не содержит `laya-serve`, проверьте публикацию upstream;
установка development checkout возможна командой
`.venv/bin/python -m pip install 'laya[serve] @ git+https://github.com/NandhaKishorM/laya.git'`,
но для воспроизводимого deployment закрепите проверенный commit вместо плавающего main.
Здесь сервер и веса **не устанавливались**, эти команды основаны на upstream, а не на smoke test модели.

В другом терминале:

```sh
curl --fail http://localhost:8000/health
printf 'PKIX path building failed\nrequest completed\n' | \
  lg --model english 'Does this line describe a PKIX or certificate error?'
```

`/health` возвращает `status`, `loaded`, `device`; probe не требует bearer даже при включённой
аутентификации. При preload startup занимает время загрузки; не объявляйте readiness раньше health.

## GPU и production

Установите подходящий CUDA build PyTorch для своей карты/драйвера вместо CPU wheel,
затем `laya[serve]`. Замените `LAYA_DEVICE=cpu` на `cuda`, измерьте RSS/VRAM на типичных
и максимальных строках. Не запускайте много Uvicorn workers без расчёта: каждый может
держать собственные веса. Текущий endpoint вызывает синхронный `router.predict` внутри
async handler; увеличение HTTP concurrency само по себе не означает batching/ускорение.

Для удалённого доступа: TLS reverse proxy, ограничение размера/частоты запросов, firewall,
`LAYA_API_KEY` на сервере и `LG_API_KEY` у клиента. Не открывайте unauthenticated API в интернет.
Сохраняйте HF-кеш на постоянный диск; фиксируйте зависимости и ревизии весов; наблюдайте
latency/errors/RSS/VRAM. Храните секреты вне Git. `LG_API_URL` должен содержать полный endpoint.

## Hosted API — только явный выбор

README upstream предлагает `https://api.impossibl.com/v1/systemone` с bearer-ключом и
`model=convaiinnovations/laya`; multilingual endpoint использует
`convaiinnovations/laya-multilingual`. Условия бесплатности, лимиты и доступность могут меняться.
Вызов ниже отправит текст провайдеру; сначала проверьте допустимость этого для ваших данных:

```sh
export LG_API_URL=https://api.impossibl.com/v1/systemone
export LG_MODEL=convaiinnovations/laya
# Установите LG_API_KEY безопасным способом, не коммитьте его.
lg 'Does this line describe a certificate error?' sanitized-example.log
```

## Контракт клиента

```json
{
  "state": "PKIX path building failed",
  "questions": {
    "match": {
      "type": "noul",
      "instructions": "Does this line describe a certificate error?"
    }
  },
  "model": "english"
}
```

`model` опционален. Локальный сервер распознаёт checkpoint aliases; неизвестное имя может
привести к auto-routing, а не к ошибке. Hosted API имеет собственные model IDs.

Для одиночной оценки клиент принимает `{"answers":{"match":{"noul":0.92}}}`.
При оценке границы state содержит `current_fragment` и `next_record`, а `questions`
содержит четыре независимых `noul`-вопроса: `same_event`, `useful_context`, `match_next`
и `match_extended`. Ответы — отдельные `answers.<id>.noul`; все четыре обязательны
и должны быть конечными числами в [0,1]. Пропущенный/нечисловой/выходящий за диапазон
ответ отклоняет весь групповой результат до изменения фрагмента. Дополнительные поля
ответа игнорируются. Строки, bool, пропущенные поля, некорректный JSON, HTTP non-2xx,
redirects, тайм-аут и ответ свыше 64 KiB — ошибки, не отсутствие совпадений. `choice`/
`score` не преобразуются в булевы решения автоматически.

## Воспроизводимая проверка качества группировки

В `openspec/changes/add-semantic-fragment-search/experiment/` находится небольшой полностью
синтетический размеченный набор, runner и честный статус измерений. Запуск требует явно
выбранного и разрешённого Laya endpoint: укажите `LG_API_URL` (полный `/v1/systemone`),
точный `LG_MODEL` и при необходимости локально настроенный `LG_API_KEY`, затем выполните
`cargo build --locked --release` и `experiment/run.sh`. Runner проверяет `/health` перед
отправкой логов, фиксирует outputs и latency по каждому кейсу и не устанавливает/запускает
сервер. Не используйте личные логи без разрешения.

Отчёт должен содержать версию `lg`, checkpoint/model и ревизию (если доступна), параметры,
hash набора, число HTTP-запросов с указанием способа подсчёта и latency, а также ожидаемые и фактические
границы/отборы на всех примерах. `experiment/RESULTS.md` содержит реальный запуск только
на синтетических данных, обнаруженные ошибки группировки и опубликованные сведения
Hugging Face (`convaiinnovations/laya`: 421M params, Laya agent `max_len=512`, head budget
192). По явному решению пользователя, эти опубликованные параметры приняты для эксперимента
с localhost на веру; это не runtime attestation. Новый синтетический пример проверен real
inference: endpoint usage сообщил 512 input tokens. Старый 124-byte edge case всё ещё
проверяет только client byte limit. Ни этот контекстный прогон, ни небольшой набор не
доказывают общее качество модели. Wiremock/scripted mock проверяет протокол, но не заменяет
inference.
Wiremock/scripted mock проверяет протокол, но не заменяет inference.
