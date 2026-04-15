# C4 Архитектура: LLM Generation Service

## Level 1: System Context

```
┌─────────────────────────────────────────────────────────────┐
│                     Пользователь                            │
│  (клиентское приложение / IDE / API)                      │
└──────────────────────┬──────────────────────────────────────┘
                       │ HTTP / REST
                       ▼
┌─────────────────────────────────────────────────────────────┐
│            LLM Generation Service                          │
│  (FastAPI:8080)                                             │
│                                                             │
│  • Принимает задачу на генерацию Lua-кода                 │
│  • Генерирует план → код → валидирует → критикует         │
│  • Возвращает готовый Lua-код                              │
└──────────────────────┬──────────────────────────────────────┘
                       │
        ┌──────────────┼──────────────┐
        ▼              ▼              ▼
   ┌─────────┐   ┌──────────┐   ┌─────────┐
   │ Ollama  │   │Sandbox   │   │ Qdrant  │
   │ :11434 │   │Service   │   │ :6333   │
   └─────────┘   └──────────┘   └─────────┘
```

## Level 2: Containers

### 2.1 Main Application (FastAPI)

```
┌──────────────────────────────────────────────────────────────────┐
│  llm-service/app/main.py (FastAPI)                              │
│                                                                  │
│  ├── GET  /health              → health check                    │
│  └── POST /generate            → generate code (core API)       │
│                                                                  │
│  Session State Machine:                                          │
│  GENERATING_PLAN → AWAITING_PLAN_CONFIRMATION                   │
│     → GENERATING_CODE → AWAITING_CODE_APPROVAL → DONE           │
│                                                                  │
│  In-memory Session Store:                                       │
│  sessions: dict[str, SessionData]                               │
└────────────────────────────────────────┬───────────────────────┘
                                          │
         ┌───────────────┬────────────────┼───────────────┐
         ▼               ▼                ▼               ▼
   ┌───────────┐  ┌──────────��─┐  ┌────────────┐  ┌──────────┐
   │Pipeline  │  │JsonParser  │  │SandboxClient│ │RAG Func  │
   │           │  │            │  │            │  │          │
   │_gen_plan │  │extract_ctx │  │validate    │  │build_rag │
   │_gen_code │  │clean_task  │  │send_code   │  │context   │
   │_critique │  │            │  │            │  │          │
   └───────────┘  └────────────┘  └────────────┘  └──────────┘
```

### 2.2 Generation Pipeline

```
┌──────────────────────────────────────────────────────────────────┐
│  GenerationPipeline (ollama_client + prompts)                   │
│                                                                  │
│  async _generate_plan(task) → план                               │
│      └─> prompts.build_architect_messages()                     │
│          └─> OllamaClient.send_request()                         │
│                                                                  │
│  async _generate_code(plan, task, critic_feedback?) → код      │
│      └─> prompts.build_coder_messages()                         │
│          └─> OllamaClient.send_request()                         │
│                                                                  │
│  async _critique_code(code) -> "CODE_OK" / замечания              │
│      └─> prompts.build_critic_messages()                        │
│          └─> OllamaClient.send_request()                         │
└──────────────────────────────────────────────────────────────────┘
```

### 2.3 Ollama Client

```
┌──────────────────────────────────────────────────────────────────┐
│  OllamaClient                                                    │
│                                                                  │
│  • model: qwen2.5-coder:7b (configurable)                        │
│  • host: http://ollama:11434 (configurable)                      │
│                                                                  │
│  async send_request(messages, keep_alive=300) -> response         │
│      └─> POST /api/chat                                         │
│      └─> streaming=False                                        │
└──────────────────────────────────────────────────────────────────┘
```

### 2.4 Sandbox Client

```
┌──────────────────────────────────────────────────────────────────┐
│  SandboxClient                                                  │
│                                                                  │
│  • url: http://sandbox-service:6778                             │
│                                                                  │
│  async send_code_for_validation(code, context) -> sandbox_resp   │
│      └─> POST /validate                                          │
│                                                                  │
│  extract_validation_feedback(response) -> True / error_msg   │
│      └─> parse JSON response                                     │
└──────────────────────────────────────────────────────────────────┘
```

### 2.5 RAG Functions

```
┌──────────────────────────────────────────────────────────────────┐
│  RAG Functions                                                  │
│                                                                  │
│  async build_rag_context(plan) -> context_text                   │
│      ├─> embed(plan) via embeddings service                    │
│      ├─> search Qdrant (top-k=1)                               │
│      └─> concat results                                         │
│                                                                  │
│  Qdrant:                                                        │
│  • host: http://qdrant:6333                                     │
│  • collection: lua_patterns                                    │
│  • embedding model: bge-m3                                      │
└──────────────────────────────────────────────────────────────────┘
```

## Level 3: Component Details

### 3.1 Session State Machine

| State | Transition | User Action | Next State |
|-------|------------|------------|-----------|
| GENERATING_PLAN | create session | send task | AWAITING_PLAN_CONFIRMATION |
| AWAITING_PLAN_CONFIRMATION | "подтвердить" | confirm | GENERATING_CODE |
| AWAITING_PLAN_CONFIRMATION | feedback | revise plan | AWAITING_PLAN_CONFIRMATION |
| GENERATING_CODE | sandboxes OK, critic OK | — | AWAITING_CODE_APPROVAL |
| AWAITING_CODE_APPROVAL | "подтвердить" | confirm | DONE |
| AWAITING_CODE_APPROVAL | feedback | revise code | AWAITING_CODE_APPROVAL |
| DONE | any | — | DONE |

### 3.2 Validation Flow

```
┌─────────────────────────────────────────────────────────────────┐
│  Code Validation Pipeline                                      │
│                                                                  │
│  1. Generate code → raw_lua                                     │
│                                                                  │
│  2. Sandbox (Rust interpreter)                                 │
│     ┌─ retries = CODE_RETRIES_SANDBOX (20)                     │
│     │  ├─ send_code_for_validation(code, context)             │
│     │  ├─ success? → continue                                  │
│     │  └─ error? → regenerate with feedback → retry           │
│     │                                                       │
│     └─ after retries: return (success=False, feedback)         │
│                                                                  │
│  3. Ollama Critic (logic, security, performance)               │
│     ┌─ _critique_code(code, rag_context)                       │
│     ├─ result == CONFIRM_WORD? → continue                       │
│     └─ result != CONFIRM_WORD? → return (success=False, feedback) │
│                                                                  │
│  4. Return to user for final approval                         │
└─────────────────────────────────────────────────────────────────┘
```

### 3.3 Configuration

```python
# Environment variables (config.py)
GENERATION_MODEL = "qwen2.5-coder:7b"    # LLM for generation
EMBEDDING_MODEL  = "bge-m3"              # For RAG

CONFIRM_WORD      = "CODE_OK"              # Word for LLM approval
CODE_RETRIES_MODEL = 2                      # Max retries for LLM fixes
CODE_RETRIES_SANDBOX = 20                   # Max sandbox retries

OLLAMA_URL        = "http://ollama:11434"
SANDBOX_SERVICE_URL = "http://sandbox-service:6778"
QDRANT_URL       = "http://qdrant:6333"
QDRANT_COLLECTION = "lua_patterns"
```

## Level 4: Code Map

```
llm-service/app/
├── main.py              # FastAPI app, session state machine
├── pipeline.py         # GenerationPipeline orchestrator
├── ollama_client.py    # Ollama API client
├── sandbox_client.py    # Sandbox service client
├── rag_func.py         # RAG context builder
├── prompts.py         # Prompt templates for LLM
├── json_input_parser.py # Task parser
├── models.py          # Pydantic models
├── config.py          # Configuration
└── __pycache__/      # Generated bytecode
```

## Deployment

```yaml
# docker-compose.yml
services:
  llm-service:
    build: ./llm-service
    ports: [8080:8080]
    environment:
      - OLLAMA_URL=http://ollama:11434
      - SANDBOX_SERVICE_URL=http://sandbox-service:6778
      - QDRANT_URL=http://qdrant:6333

  ollama:
    image: ollama/ollama:latest
    ports: [11434:11434]

  sandbox-service:
    image: sandbox-service:latest
    ports: [6778:6778]

  qdrant:
    image: qdrant/qdrant:latest
    ports: [6333:6333]
```