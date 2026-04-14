# LuaForge AI

> **AI-powered Lua code generation с многоуровневой безопасностью и агентным self-correction loop**

Система, которая принимает задачу на естественном языке (в т.ч. на русском), генерирует план реализации, пишет Lua-код, валидирует его в защищённой песочнице, подвергает код ревью LLM-критиком и итеративно исправляет ошибки — до одобрения пользователем.

Проект создан для хакатона **True Tech Hack 2026**.

---

## 🚀 Киллер-фичи

### 🔒 Многоуровневая система безопасности

Система защищает от вредоносного или опасного Lua-кода на **трёх уровнях**:

| Уровень | Механизм | Что проверяет |
|---------|----------|---------------|
| **1. Text-level** | Regex-паттерны | 10 опасных паттернов: `rm -rf`, `sudo rm`, `mkfs`, `dd if=`, `:(){` (fork bomb), `shutdown`, `reboot`, `curl \| sh`, `wget \| sh` |
| **2. AST-level** | Tree-sitter парсинг | 34 запрещённых функции, включая `os.execute`, `io.open`, `require`, `debug`, `setmetatable`, `_G`, `_ENV`, `string.char` (обфускация), `coroutine.wrap` (обход хуков). Защита от записи через `obj["method"]` и `obj:method` |
| **3. Runtime sandbox** | mlua (Lua 5.4) | `os`, `io`, `package`, `debug`, `coroutine` → `nil`; лимит памяти 8 MB; instruction hook каждые 10 000 команд + hard timeout; перехват `print()`/`warn()` |

Даже если код прошёл статический анализ, он выполняется в полностью изолированной среде без доступа к файловой системе, сети и операционной системе.

### 🤖 Агентный self-correction loop

Система **автономно исправляет ошибки** без участия пользователя:

1. Сгенерированный код проходит через Rust-песочницу
2. Если песочница обнаруживает ошибку — код автоматически отправляется обратно в LLM с фидбэком
3. До **5 попыток** автоматического исправления ошибок песочницы
4. После прохождения песочницы код проверяет **LLM-критик** (до 2 итераций исправлений)
5. Только после всех проверок код показывается пользователю

### 🧠 Три роли LLM (multi-agent)

| Роль | Промпт | Задача |
|------|--------|--------|
| **Systems Architect** | «Expert Systems Architect» | Разбивает задачу пользователя на пошаговый план реализации. Без кода. |
| **Senior Lua Developer** | «Senior Lua Developer» | Пишет чистый, эффективный Lua-код по плану. Только код, без объяснений. |
| **QA & Security Auditor** | «Senior QA Engineer and Security Auditor» | Ревью кода: синтаксис, логика, производительность, безопасность. Отвечает `CODE_OK` или список проблем. |

### 🖥️ Terminal UI

Интерактивный терминальный клиент (`llm-tui`) на Rust с:
- Историей сообщений (plans, code, feedback)
- Копированием последнего кода в буфер обмена (`F3`)
- Скроллом истории (`↑`/`↓`)
- Очисткой сессии (`F4`)
- Статус-баром с текущим состоянием

### 📦 Контекст workflow-платформы

Пользователь может вложить JSON-контекст прямо в задачу:

```
Очисти значения переменных ID, ENTITY_ID, CALL
{
  "wf": {
    "vars": {
      "RESTbody": { "result": [...] }
    },
    "initVariables": {}
  }
}
```

Система автоматически извлечёт контекст, распарсит его и передаст в Lua-песочницу как глобальную таблицу `wf`, доступную в генерируемом коде.

---

## 🏗️ Архитектура

### Схема сервисов

```
┌─────────────────────────────────────────────────────────┐
│                    Пользователь                          │
│              (llm-tui / curl / HTTP API)                 │
└──────────────────────┬──────────────────────────────────┘
                       │ POST /generate
                       ▼
┌─────────────────────────────────────────────────────────┐
│                   llm-service                            │
│              Python 3.12 + FastAPI :8080                 │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────────┐ │
│  │ Architect│→ │  Coder   │→ │ Sandbox → Critic Loop  │ │
│  └──────────┘  └──────────┘  └────────────────────────┘ │
└───┬────────────────────────┬────────────────────────────┘
    │                        │
    │ Ollama API             │ POST /pipeline
    ▼                        ▼
┌──────────────┐    ┌──────────────────────────────────┐
│    Ollama    │    │        sandbox-service            │
│  :11434      │    │       Rust + axum :6778          │
│              │    │  ┌───────┐ ┌────────┐ ┌────────┐ │
│ qwen2.5-coder│    │  │ Parse │→│ Safety │→│Execute │ │
│   :7b        │    │  │(tree- │  │(text+  │  │(mlua   │ │
│              │    │  │sitter)│  │ AST)   │  │Lua 5.4)│ │
└──────────────┘    │  └───────┘ └────────┘ └────────┘ │
                    └──────────────────────────────────┘
```

### Сервисы

| Сервис | Технологии | Порт | Описание |
|--------|-----------|------|----------|
| **ollama** | Go, Ollama | `11434` | Локальный LLM-сервер. Загружает и обслуживает модель `qwen2.5-coder:7b`. Модель хранится в Docker volume `ollama_data` и кэшируется на 24 часа. |
| **ollama-init** | Shell-скрипт | — | Init-контейнер: ждёт healthcheck Ollama, проверяет наличие модели, при необходимости скачивает. Завершается после запуска. |
| **sandbox-service** | Rust 1.88, axum, tokio, mlua, tree-sitter | `6778` | Защищённая песочница для Lua. Парсит AST, проверяет безопасность, выполняет код. Endpoint: `POST /pipeline`. |
| **llm-service** | Python 3.12, FastAPI, uvicorn, httpx, pydantic | `8080` | Оркестратор генерации. Session-based state machine, multi-agent pipeline. Endpoints: `POST /generate`, `GET /health`. |
| **llm-tui** | Rust, ratatui, crossterm, reqwest, arboard | TUI | Терминальный UI-клиент для интерактивной работы с llm-service. |

---

## 🛠️ Технологический стек

| Слой | Технология |
|------|-----------|
| **LLM** | Ollama + Qwen2.5-Coder 7B (code-specialized 7B model) |
| **Backend (оркестрация)** | Python 3.12, FastAPI, uvicorn, httpx, pydantic 2 |
| **Backend (песочница)** | Rust 1.88, axum 0.8, tokio 1, mlua 0.10 (Lua 5.4, vendored), tree-sitter 0.25 + tree-sitter-lua 0.5 |
| **Frontend (TUI)** | Rust, ratatui 0.29, crossterm 0.28, reqwest 0.12, arboard 3 |
| **Контейнеризация** | Docker + Docker Compose |
| **CI/CD** | GitLab (git.truetecharena.ru) |

---

## ⚡ Быстрый старт

### Требования

- **Docker** 20.10+
- **Docker Compose** v2+
- **~5 GB** свободного места (модель qwen2.5-coder:7b ~4 GB)
- **16 GB RAM** рекомендуется (модель 7B требует ~8 GB)

### Запуск всех сервисов

```bash
git clone <repository-url>
cd task-repo
docker compose up --build
```

При первом запуске `ollama-init` автоматически загрузит модель `qwen2.5-coder:7b` (~4 GB). Это займёт несколько минут.

Сервисы будут доступны:

| Сервис | URL |
|--------|-----|
| LLM-service API | http://localhost:8080/generate |
| Health check | http://localhost:8080/health |
| Sandbox-service | http://localhost:6778/pipeline |
| Ollama API | http://localhost:11434 |

### Использование через curl

```bash
# Шаг 1: Создать сессию и получить план
curl -s -X POST http://localhost:8080/generate \
  -H "Content-Type: application/json" \
  -d '{"task": "Напиши Lua-функцию, которая очищает RESTbody от лишних ключей, оставляя только ID, ENTITY_ID, CALL"}' | jq

# Шаг 2: Подтвердить план
curl -s -X POST http://localhost:8080/generate \
  -H "Content-Type: application/json" \
  -d '{"session_id": "<session_id>", "user_response": "Подтвердить"}' | jq

# Шаг 3: Одобрить код (или указать правки)
curl -s -X POST http://localhost:8080/generate \
  -H "Content-Type: application/json" \
  -d '{"session_id": "<session_id>", "user_response": "Подтвердить"}' | jq
```

### Использование через TUI

Для запуска TUI отдельно (нужен Rust 1.88+):

```bash
cd llm-tui
cargo run
```

Или через Docker Compose:

```bash
docker compose -f docker-compose.yml -f docker-compose.tui.yml up --build llm-tui
```

**Горячие клавиши TUI:**

| Клавиша | Действие |
|---------|----------|
| `Enter` | Отправить сообщение / подтвердить |
| `q` / `Esc` | Выход |
| `F3` | Копировать последний код в буфер |
| `F4` | Очистить историю, новая сессия |
| `↑` / `↓` | Скролл истории |

---

## 📡 API Reference

### LLM Service (`:8080`)

#### `POST /generate`

Основной endpoint для генерации кода. Реализует session-based state machine.

**Request:**

```json
{
  "session_id": "uuid (опционально, для новых сессий)",
  "task": "Описание задачи на естественном языке",
  "user_response": "Подтвердить | текст правок",
  "llm_validation": true
}
```

| Поле | Тип | Обязательное | Описание |
|------|-----|:------------:|----------|
| `session_id` | `string?` | Нет | ID сессии. Если не указан — создаётся новая. |
| `task` | `string` | Да* | Описание задачи. Обязателен для новой сессии. |
| `user_response` | `string` | Нет | `"Подтвердить"` для одобрения, или текст правок. |
| `llm_validation` | `bool` | Нет | Включить LLM-критик после генерации (default: `true`). |

**Response:**

```json
{
  "session_id": "abc-123",
  "state": "awaiting_plan_confirmation",
  "plan": "1. Распарсить RESTbody\n2. Очистить...",
  "code": null,
  "sandbox_feedback": null,
  "message": "План сгенерирован. Подтвердите или укажите исправления."
}
```

**Состояния (state):**

| State | Описание | Что делать дальше |
|-------|----------|-------------------|
| `generating_plan` | Генерация плана (внутреннее) | — |
| `awaiting_plan_confirmation` | План готов | `"Подтвердить"` или правки |
| `generating_code` | Генерация кода (внутреннее) | — |
| `awaiting_code_approval` | Код готов | `"Подтвердить"` или правки |
| `done` | Код одобрен | Больше действий не требуется |

#### `GET /health`

```json
{
  "status": "ok",
  "active_sessions": 3,
  "pipeline_model": "qwen2.5-coder:7b"
}
```

### Sandbox Service (`:6778`)

#### `POST /pipeline`

Валидация и выполнение Lua-кода.

**Request:**

```json
{
  "code": "local x = 1\nprint(x)",
  "execute": true,
  "timeout": 2,
  "context": {
    "wf": { "vars": {}, "initVariables": {} }
  }
}
```

| Поле | Тип | Default | Описание |
|------|-----|:-------:|----------|
| `code` | `string` | — | Lua-код для проверки |
| `execute` | `bool` | `false` | Выполнять ли код |
| `timeout` | `int` | `2` | Timeout выполнения (1–10 сек) |
| `context` | `object?` | `{"wf": {...}}` | Контекст workflow-платформы |

**Response:**

```json
{
  "status": "ok",
  "source_code": "local x = 1\nprint(x)",
  "output": "1",
  "logs": ["[exec] starting, timeout=2s", "[exec] memory used: 24576 bytes"],
  "warnings": [],
  "error_detail": null,
  "ast_analysis": {
    "function_calls": ["print"],
    "has_dangerous_patterns": false,
    "has_forbidden_calls": false
  },
  "execution_stats": {
    "memory_used_bytes": 24576,
    "execution_time_ms": 12
  }
}
```

**Статусы (status):**

| Status | Описание |
|--------|----------|
| `ok` | Код валиден |
| `syntax_error` | Ошибка синтаксиса |
| `safety_error` | Обнаружены опасные паттерны/функции |
| `runtime_error` | Ошибка выполнения |
| `timeout` | Превышен timeout |

---

## ⚙️ Конфигурация

### llm-service

| Переменная | Default | Описание |
|------------|---------|----------|
| `OLLAMA_URL` | `http://ollama:11434` | URL Ollama API |
| `SANDBOX_SERVICE_URL` | `http://sandbox-service:6778` | URL sandbox-service |
| `GENERATION_MODEL` | `qwen2.5-coder:7b` | Имя модели Ollama |
| `CONFIRM_WORD` | `CODE_OK` | Ключевое слово принятия кода критиком |
| `MAX_RETRIES` | `2` | Макс. итераций критика (план) |
| `CODE_RETRIES_COUNT` | `5` | Макс. авто-исправлений после sandbox |
| `CODE_RETRIES_MODEL` | `2` | Макс. итераций критика (код) |
| `CODE_RETRIES_SANDBOX` | `20` | Макс. retry sandbox внутри validate_code |
| `HOST` | `0.0.0.0` | Host для uvicorn |
| `PORT` | `8080` | Port для uvicorn |

### sandbox-service

| Переменная | Default | Описание |
|------------|---------|----------|
| `RUST_LOG` | `info` | Уровень логирования (trace, debug, info, warn, error) |

### llm-tui

| Переменная | Default | Описание |
|------------|---------|----------|
| `LLM_SERVICE_URL` | `http://localhost:8080` | URL llm-service |

### Ollama

| Переменная | Default | Описание |
|------------|---------|----------|
| `OLLAMA_KEEP_ALIVE` | `24h` | Время удержания модели в памяти |
| `MODEL_NAME` | `qwen2.5-coder:7b` | Модель для инициализации (ollama-init) |

---

## 🔄 Pipeline генерации кода

### State Machine

```
                    ┌─────────────────────┐
                    │   GENERATING_PLAN    │
                    └──────────┬──────────┘
                               │
                               ▼
                    ┌─────────────────────┐
            ┌───────│ AWAITING_PLAN       │
            │       │   CONFIRMATION      │◄──────────────┐
            │       └──────────┬──────────┘               │
            │                  │                          │
            │    ┌─────────────┴─────────────┐            │
            │    │                           │            │
            │    ▼                           ▼            │
            │  "Подтвердить"             Другой ответ     │
            │    │                           │            │
            │    ▼                           │            │
            │ ┌─────────────────────┐        │            │
            │ │ GENERATING_CODE     │        │            │
            │ └──────────┬──────────┘        │            │
            │            │                   │            │
            │            ▼                   │            │
            │ ┌─────────────────────┐        │            │
            │ │ AWAITING_CODE       │◄───────┘            │
            │ │   APPROVAL          │                     │
            │ └──────────┬──────────┘                     │
            │            │                                │
            │    ┌───────┴───────┐                        │
            │    │               │                        │
            │    ▼               ▼                        │
            │ "Подтвердить"   Другой ответ ───────────────┘
            │    │
            │    ▼
            │ ┌─────────────────────┐
            └─│        DONE         │
              └─────────────────────┘
```

### Авто-фикс цикл (внутренний)

После генерации кода запускается автоматическая валидация:

```
Сгенерированный код
       │
       ▼
┌──────────────┐     ❌ Ошибка      ┌───────────────┐
│  Sandbox     │ ──────────────────→│  LLM: "исправь│
│  validation  │                    │  с учётом     │
│  (до 5 раз)  │◄────────────────── │  feedback)    │
└──────┬───────┘                    └───────────────┘
       │ ✅ OK
       ▼
┌──────────────┐     ❌ Замечания   ┌───────────────┐
│  LLM Critic  │ ──────────────────→│  LLM: "исправь│
│  (до 2 раз)  │◄────────────────── │  с учётом     │
└──────┬───────┘                    │  замечаний)   │
       │ ✅ CODE_OK                 └───────────────┘
       ▼
   Показ пользователю
```

---

## 🛡️ Детали безопасности

### Уровень 1: Text-level паттерны

Проверка исходного кода на строковые паттерны (case-insensitive):

| Паттерн | Угроза |
|---------|--------|
| `rm -rf` / `rm -fr` | Удаление файлов |
| `sudo rm` | Удаление с root |
| `mkfs` | Форматирование ФС |
| `dd if=` | Прямой доступ к диску |
| `:(){` | Fork bomb |
| `shutdown` / `reboot` | Перезагрузка |
| `curl \| sh` / `wget \| sh` | Загрузка и исполнение из сети |

### Уровень 2: AST-level (Tree-sitter)

Парсинг Lua-кода в AST и извлечение всех вызовов функций. Проверка против **34 запрещённых функций**:

**OS-операции:** `os.execute`, `os.remove`, `os.rename`, `os.tmpname`, `os.exit`

**I/O-операции:** `io.open`, `io.popen`, `io.input`, `io.output`, `io.close`

**Динамическая загрузка:** `require`, `dofile`, `loadfile`, `load`, `loadstring`, `package.loadlib`

**Debug & metatable:** `debug`, `setmetatable`, `getmetatable`, `getfenv`, `setfenv`, `rawget`, `rawset`, `rawequal`, `debug.getinfo`

**Глобальное окружение:** `_G`, `_ENV`

**Обфускация:** `string.char`, `string.byte` (попытки собрать имена функций из char-кодов)

**Обход sandbox:** `coroutine.wrap` (обход instruction hook), `dostring`, `getfenv(0)`

**Защита от обхода:** проверяются все формы вызова:
- Dot notation: `os.execute()`
- Bracket notation: `os["execute"]()`
- Method notation: `os:execute()`
- Вложенные: `os.execute.subfunction()`

### Уровень 3: Runtime sandbox (mlua)

Даже если код прошёл статический анализ, он выполняется в изолированной среде:

- **Nil-out globals:** `os`, `io`, `package`, `debug`, `coroutine` → `nil`
- **Лимит памяти:** 8 MB (при превышении — `MemoryLimit` error)
- **Instruction hook:** каждые 10 000 команд проверка timeout
- **Hard timeout:** tokio timeout + 200ms буфер
- **Перехват print/warn:** вывод перехватывается и логируется
- **JSON context injection:** `wf.vars` и `wf.initVariables` доступны как Lua-таблицы

---

## 📁 Структура проекта

```
task-repo/
├── docker-compose.yml              # Основная оркестрация (ollama + sandbox + llm)
├── docker-compose.tui.yml          # Override для запуска TUI
├── run.sh                          # Альтернативный запуск (только sandbox + llm, без Ollama)
│
├── llm-service/                    # Python FastAPI — оркестратор генерации
│   ├── Dockerfile
│   ├── requirements.txt
│   ├── .env.example
│   └── app/
│       ├── main.py                 # FastAPI app, session state machine, endpoints
│       ├── config.py               # Конфигурация из env vars
│       ├── pipeline.py             # GenerationPipeline (plan → code → critique)
│       ├── prompts.py              # Системные промпты для Architect, Coder, Critic
│       ├── ollama_client.py        # Async HTTP клиент для Ollama API
│       ├── sandbox_client.py       # HTTP клиент для sandbox-service + нормализация ответов
│       ├── models.py               # Pydantic-модели (SandboxResponse, AST analysis, etc.)
│       ├── json_input_parser.py    # Парсинг JSON-контекста из задачи пользователя
│       ├── parse_lua_to_json.py    # Утилита: Lua → JSON wrapper
│       └── run_model.py            # Standalone тестовый скрипт
│
├── sandbox-service/                # Rust axum — защищённая песочница
│   ├── Cargo.toml
│   ├── Dockerfile
│   └── src/
│       ├── main.rs                 # Axum router, запуск на :6778
│       ├── models.rs               # PipelineStatus, PipelineRequest/Response, AstAnalysis, ExecutionStats
│       ├── ast/
│       │   ├── mod.rs              # Module declarations
│       │   ├── parser.rs           # parse_lua_code (tree-sitter), recursive AST walk
│       │   ├── extractor.rs        # extract_function_calls — обход AST
│       │   └── safety.rs           # DANGEROUS_TEXT_PATTERNS + FORBIDDEN_AST_CALLS
│       ├── executor/
│       │   ├── mod.rs              # Module declarations
│       │   └── sandbox.rs          # mlua sandbox, execute_lua_code, context injection
│       └── routes/
│           ├── mod.rs              # Module declarations
│           └── pipeline.rs         # handle_pipeline — Parse → Safety → Execute
│
└── llm-tui/                        # Rust ratatui — терминальный UI
    ├── Cargo.toml
    ├── Dockerfile
    └── src/
        ├── main.rs                 # Terminal init, event loop, key bindings
        ├── app.rs                  # App state machine, send_message, scroll, copy
        ├── api.rs                  # HTTP клиент для llm-service /generate
        ├── config.rs               # Config из LLM_SERVICE_URL env var
        └── ui.rs                   # ratatui rendering: history, input, status bar
```

---

## 🔧 Разработка

### Запуск отдельных сервисов

**Только sandbox-service и llm-service (без Ollama):**

```bash
bash run.sh
```

Этот скрипт собирает Docker-образы и запускает сервисы, используя `host.docker.internal` для межсервисной коммуникации.

**Добавить TUI к основному compose:**

```bash
docker compose -f docker-compose.yml -f docker-compose.tui.yml up --build
```

### Локальная разработка (без Docker)

**llm-service (Python):**

```bash
cd llm-service
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt

# Запуск (нужен работающий Ollama и sandbox-service)
OLLAMA_URL=http://localhost:11434 \
SANDBOX_SERVICE_URL=http://localhost:6778 \
uvicorn app.main:app --host 0.0.0.0 --port 8080 --reload
```

**sandbox-service (Rust):**

```bash
cd sandbox-service
cargo run
# или
cargo build --release && ./target/release/sandbox-service
```

**llm-tui (Rust):**

```bash
cd llm-tui
LLM_SERVICE_URL=http://localhost:8080 cargo run
```

### Остановка и очистка

```bash
# Остановить все контейнеры
docker compose down

# Остановить и удалить volumes (модель Ollama будет загружена заново)
docker compose down -v

# Очистить старые образы
docker compose down --rmi local
```

---

## 🧪 Примеры задач

### Простая задача

```
Напиши Lua-функцию, которая принимает таблицу data и возвращает
новую таблицу, содержащую только ключи id, name, email.
```

### С контекстом workflow

```
Для полученных данных из REST-запроса очисти значения переменных
ID, ENTITY_ID, CALL, оставив только их.

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

---

## 📝 Примечания

- **Модель не выгружается** 24 часа (`OLLAMA_KEEP_ALIVE=24h`) — повторные запросы обрабатываются быстро
- **Сессии хранятся в памяти** — при рестарте llm-service все сессии сбрасываются
- **Sandbox требует `privileged: true`** в Docker Compose для корректной работы mlua
- **Требования к RAM:** минимум 8 GB для модели 7B + 8 GB лимит песочницы

---

## 📄 Лицензия

Проект создан в рамках хакатона True Tech Hack 2026.
