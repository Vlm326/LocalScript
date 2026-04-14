# LuaForge AI

[![pipeline status](https://git.truetecharena.ru/tta/true-tech-hack2026-localscript/710/task-repo/-/badges/main/pipeline.svg)](https://git.truetecharena.ru/tta/true-tech-hack2026-localscript/710/task-repo/-/pipelines)
[![coverage report](https://git.truetecharena.ru/tta/true-tech-hack2026-localscript/710/task-repo/-/badges/main/coverage.svg)](https://git.truetecharena.ru/tta/true-tech-hack2026-localscript/710/task-repo/-/graphs/main/charts)
[![latest release](https://git.truetecharena.ru/tta/true-tech-hack2026-localscript/710/task-repo/-/badges/release.svg)](https://git.truetecharena.ru/tta/true-tech-hack2026-localscript/710/task-repo/-/releases)

[![docker](https://img.shields.io/badge/Docker-Ready-2496ED?logo=docker&logoColor=white)](https://www.docker.com/)
[![docker compose](https://img.shields.io/badge/Docker%20Compose-v2-2496ED?logo=docker&logoColor=white)](https://docs.docker.com/compose/)
[![python](https://img.shields.io/badge/Python-3.12-3776AB?logo=python&logoColor=white)](https://www.python.org/)
[![fastapi](https://img.shields.io/badge/FastAPI-0.115%2B-009688?logo=fastapi&logoColor=white)](https://fastapi.tiangolo.com/)
[![rust](https://img.shields.io/badge/Rust-1.88-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![axum](https://img.shields.io/badge/Axum-0.8-000000)](https://docs.rs/axum/)
[![tokio](https://img.shields.io/badge/Tokio-1.x-000000)](https://tokio.rs/)
[![mlua](https://img.shields.io/badge/mlua-0.10-6E4C13)](https://github.com/mlua-rs/mlua)
[![tree sitter](https://img.shields.io/badge/tree--sitter-0.25-2D6CDF)](https://tree-sitter.github.io/tree-sitter/)
[![ollama](https://img.shields.io/badge/Ollama-local%20LLM-000000)](https://ollama.com/)
[![qdrant](https://img.shields.io/badge/Qdrant-vector%20DB-FF4E00)](https://qdrant.tech/)
[![ratatui](https://img.shields.io/badge/ratatui-0.29-5E2B97)](https://ratatui.rs/)

Сервис генерации Lua-кода по описанию на естественном языке с интерактивным подтверждением плана/кода и многоуровневой валидацией в изолированной песочнице.

Ключевая идея: пользователь формулирует задачу → сервис генерирует план → пользователь подтверждает/правит план → сервис генерирует Lua-код → код проходит sandbox-валидацию и LLM-ревью → пользователь подтверждает/правит результат.

Проект создан для True Tech Hack 2026.

## Содержание

- [Возможности](#возможности)
- [Компоненты](#компоненты)
- [Архитектура](#архитектура)
- [Быстрый старт (Docker Compose)](#быстрый-старт-docker-compose)
- [Запуск TUI (Docker Compose)](#запуск-tui-docker-compose)
- [HTTP API](#http-api)
  - [GET /health](#get-health)
  - [POST /generate](#post-generate)
  - [Примеры curl](#примеры-curl)
- [Контекст в задаче (JSON)](#контекст-в-задаче-json)
- [Конфигурация](#конфигурация)
- [Операционные заметки](#операционные-заметки)
- [Структура репозитория](#структура-репозитория)
- [Локальная разработка (без Docker)](#локальная-разработка-без-docker)
- [Лицензия](#лицензия)

## Возможности

- Session-based state machine с ручным подтверждением плана и кода (см. `llm-service/app/main.py`).
- Авто-исправления после ошибок песочницы (повторная генерация кода с фидбэком от валидатора).
- Многоуровневая безопасность исполнения Lua:
  - статические проверки (опасные текстовые паттерны и запрещённые вызовы на уровне AST),
  - runtime sandbox (ограничение памяти, таймаут, отключение доступа к `os/io/package/debug/coroutine`, перехват `print/warn`).
- Опциональный RAG-контекст для критика через Qdrant + embeddings-сервис (если недоступно — шаг пропускается).
- Терминальный клиент `llm-tui` для интерактивного диалога с сервисом.

## Компоненты

Состав репозитория и роль сервисов:

- `llm-service` (FastAPI, порт `8080`) — HTTP API и state machine сессий, оркестрация генерации/валидации.
- `sandbox-service` (Rust, порт `6778`) — статическая и runtime-валидация Lua-кода (включая ограничения окружения исполнения).
- `ollama` (порт `11434`) — локальный LLM-сервер для генерации и критика.
- `qdrant` (порт `6333`) — векторная БД для RAG (опционально; используется для подсказок/политик в критике).
- `llm-tui` — терминальный клиент для интерактивной работы с `llm-service`.

## Архитектура

Высокоуровневый поток:

1) Клиент (curl/TUI) отправляет задачу в `llm-service` (`POST /generate`).
2) `llm-service` генерирует план → возвращает план пользователю на подтверждение.
3) После подтверждения плана `llm-service` генерирует Lua-код и отправляет его в `sandbox-service` на валидацию/исполнение.
4) При включённом `llm_validation` код дополнительно проходит LLM-критика; затем возвращается пользователю на финальное подтверждение.

Инварианты и ограничения:

- `llm-service` хранит сессии в памяти процесса (без внешнего стореджа).
- `sandbox-service` является источником истины по безопасности исполнения (проверки + sandbox runtime).

## Быстрый старт (Docker Compose)

Требования:

- Docker 20.10+
- Docker Compose v2+
- Достаточно места под модель (первый запуск скачает LLM-модель через `ollama-init`)

Запуск всего стека (сборка локально и старт контейнеров):

```bash
docker compose up --build
```

Проверка готовности API:

```bash
curl -s http://localhost:8080/health
```

Полезные порты (по умолчанию):

- `http://localhost:8080` — `llm-service`
- `http://localhost:6778` — `sandbox-service`
- `http://localhost:11434` — `ollama`
- `http://localhost:6333` — `qdrant`

Остановка:

```bash
docker compose down
```

## Запуск TUI (Docker Compose)

TUI запускается отдельной командой поверх уже поднятого стека.

1) Собрать и поднять сервисы:

```bash
docker compose up --build
```

2) В другом терминале запустить TUI:

```bash
docker compose run --rm --no-deps -it llm-tui
```

Переменная `LLM_SERVICE_URL` внутри контейнера TUI уже настроена на `http://llm-service:8080` (см. `docker-compose.yml`).

## HTTP API

Базовый URL: `http://localhost:8080`.

### GET /health

Health endpoint `llm-service`.

Пример:

```bash
curl -s http://localhost:8080/health
```

### POST /generate

Единственный основной endpoint. Реализует session-based state machine. Запросы делаются последовательно, используя один и тот же `session_id`.

Request (JSON):

```json
{
  "session_id": "uuid (опционально)",
  "task": "описание задачи (обязательно только при создании новой сессии)",
  "user_response": "\"Подтвердить\" или текст правок",
  "llm_validation": true
}
```

Поля:

- `session_id` — если не указан, создаётся новая сессия и возвращается в ответе.
- `task` — обязателен для новой сессии; для продолжения сессии можно не передавать.
- `user_response`:
  - на этапе плана: `"Подтвердить"` подтверждает план; любой другой текст считается правками и приводит к пересборке плана;
  - на этапе кода: `"Подтвердить"` завершает сессию; любой другой текст считается правками и приводит к пересборке кода.
- `llm_validation` — включает/выключает LLM-критика после sandbox-проверки (по умолчанию `true`).

Response (JSON):

```json
{
  "session_id": "uuid",
  "state": "awaiting_plan_confirmation | awaiting_code_approval | done",
  "plan": "строка (опционально)",
  "code": "строка (опционально)",
  "sandbox_feedback": "строка (опционально)",
  "message": "текстовое пояснение"
}
```

Состояния:

- `awaiting_plan_confirmation` — вернулся план, ждём подтверждение/правки.
- `awaiting_code_approval` — вернулся код (и результаты проверок), ждём подтверждение/правки.
- `done` — код одобрен; повторные вызовы вернут финальный код для этого `session_id`.

### Примеры curl

1) Создать новую сессию и получить план:

```bash
curl -sS -X POST http://localhost:8080/generate \
  -H "Content-Type: application/json" \
  -d '{"task":"Напиши Lua-функцию, которая принимает таблицу data и возвращает новую таблицу только с ключами id, name, email."}'
```

2) Уточнить/поправить план (тот же `session_id`):

```bash
curl -sS -X POST http://localhost:8080/generate \
  -H "Content-Type: application/json" \
  -d '{"session_id":"<session_id>","user_response":"Добавь обработку отсутствующих ключей и не меняй входную таблицу."}'
```

3) Подтвердить план и получить код:

```bash
curl -sS -X POST http://localhost:8080/generate \
  -H "Content-Type: application/json" \
  -d '{"session_id":"<session_id>","user_response":"Подтвердить"}'
```

4) Попросить правки к коду:

```bash
curl -sS -X POST http://localhost:8080/generate \
  -H "Content-Type: application/json" \
  -d '{"session_id":"<session_id>","user_response":"Сделай функцию чистой (без побочных эффектов) и добавь комментарии к публичным функциям."}'
```

5) Финально подтвердить код (переводит сессию в `done`):

```bash
curl -sS -X POST http://localhost:8080/generate \
  -H "Content-Type: application/json" \
  -d '{"session_id":"<session_id>","user_response":"Подтвердить"}'
```

Опционально: выключить LLM-критика (полезно для диагностики/ускорения):

```bash
curl -sS -X POST http://localhost:8080/generate \
  -H "Content-Type: application/json" \
  -d '{"session_id":"<session_id>","user_response":"Подтвердить","llm_validation":false}'
```

## Контекст в задаче (JSON)

В `task` можно вложить JSON-контекст workflow. `llm-service` извлечёт JSON из текста, распарсит его и передаст в `sandbox-service` как таблицу `wf`.

Пример:

```text
Очисти значения переменных ID, ENTITY_ID, CALL, оставив только их.

{
  "wf": {
    "vars": {
      "RESTbody": {
        "result": [
          {"ID": 123, "ENTITY_ID": 456, "CALL": "call_1", "EXTRA": "remove_me"}
        ]
      }
    },
    "initVariables": {}
  }
}
```

## Конфигурация

`llm-service` читает переменные окружения (см. `llm-service/app/config.py`). Пример набора переменных: `llm-service/.env.example`.

Docker Compose задаёт минимум, остальное можно передать через `.env` рядом с `docker-compose.yml` или через `environment`.

Ключевые переменные:

- `GENERATION_MODEL` — модель Ollama для генерации (по умолчанию `qwen2.5-coder:7b`)
- `OLLAMA_URL` — URL Ollama (по умолчанию `http://ollama:11434`)
- `SANDBOX_SERVICE_URL` — URL sandbox (по умолчанию `http://sandbox-service:6778`)
- `CONFIRM_WORD` — ключевое слово «ОК» от критика (по умолчанию `CODE_OK`)
- `CODE_RETRIES_SANDBOX` — количество авто-попыток починки кода после ошибки песочницы (по умолчанию `20`)
- `QDRANT_URL`, `QDRANT_COLLECTION` — RAG-хранилище (опционально)
- `EMBEDDINGS_URL`, `EMBEDDING_MODEL` — сервис эмбеддингов для RAG (опционально; при недоступности RAG будет silently skipped)

## Операционные заметки

- Сессии `llm-service` хранятся в памяти процесса: при рестарте контейнера активные сессии теряются.
- `sandbox-service` запускается с `privileged: true` (см. `docker-compose.yml`). Не публикуйте sandbox наружу без отдельной оценки рисков и сетевых политик.
- Первый запуск может занять время из-за скачивания модели Ollama (`ollama-init`). Повторные старты используют volume `ollama_data`.
- Хранилище Qdrant примонтировано как bind-mount `./qdrant_data` (данные сохраняются между перезапусками).

## Структура репозитория

```text
.
├── docker-compose.yml
├── llm-service/         # FastAPI оркестратор (HTTP API, state machine)
├── sandbox-service/     # Rust sandbox (валидация/исполнение Lua)
├── llm-tui/             # Rust TUI клиент
└── docs/                # дополнительная документация (например C4)
```

## Локальная разработка (без Docker)

`llm-service` (нужны запущенные `ollama` и `sandbox-service`):

```bash
cd llm-service
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt

OLLAMA_URL=http://localhost:11434 \
SANDBOX_SERVICE_URL=http://localhost:6778 \
uvicorn app.main:app --host 0.0.0.0 --port 8080 --reload
```

`llm-tui`:

```bash
cd llm-tui
LLM_SERVICE_URL=http://localhost:8080 cargo run
```

## Лицензия

Отдельный файл лицензии в репозитории не задан. Проект создан в рамках True Tech Hack 2026.
