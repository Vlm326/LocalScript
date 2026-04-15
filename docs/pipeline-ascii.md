# Code Generation Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                           LLM CODE GENERATION PIPELINE                                   │
└─────────────────────────────────────────────────────────────────────────────────────────────┘

     ┌──────────┐      ┌──────────────┐      ┌─────────────┐      ┌────────────┐
     │  USER    │      │  FASTAPI    │      │ PIPELINE   │      │  SANDBOX  │
     │ request  │─────▶│  /generate │─────▶│ _generate  │─────▶│ (Rust)    │
     └──────────┘      └──────────────┘      │ _plan      │      └────────────┘
                                          │ _code     │            │
                                          │ _critique │            ▼
                                          └─────────────┘      ┌────────────┐
                                                           │ PASSED?   │
                                                           └────────────┘
                                                                 │
                            ┌───────────────────────────────────────┘
                            │ NO                    ▼
                            ▼                ┌──────────────────┐
                     ┌─────────────┐         │   RETRY LOOP      │
                     │  FEEDBACK  │◀───────┤ (CODE_RETRIES_  │
                     │  to LLM    │        │   SANDBOX)      │
                     └─────────────┘        └──────────────────┘
                            │
                            ▼
                  ┌─────────────────┐
                  │  USER         │
                  │  APPROVAL     │
                  │  "Подтвердить"│
                  └─────────────────┘
```

## Stages

```
┌────────────────────────────────────────────────────────────────────────────┐
│  1. PLAN GENERATION                                                      │
│  ┌──────────────────────────────────────────────────────────────────────┐ │
│  │ task ──▶ SYSTEM_ARCHITECT ──▶ Ollama (qwen2.5-coder:7b) ──▶ plan  │ │
│  └──────────────────────────────────────────────────────────────────────┘ │
│                              │                                          │
│                              ▼                                          │
│  ┌──────────────────────────────────────────────────────────────────────┐ │
│  │  2. CODE GENERATION                                                  │ │
│  │  plan ──▶ SYSTEM_CODER ──▶ Ollama ──▶ Lua code                     │ │
│  │                                      │                             │ │
│  │                                      ▼                             │ │
│  │                              ┌──────────────────┐                   │ │
│  │                              │ 3. VALIDATION    │                   │ │
│  │                              ├──────────────────┤                   │ │
│  │                              │ A. Syntax parse  │                   │ │
│  │                              │ B. Safety check  │                   │ │
│  │                              │ C. Sandbox exec  │                   │ │
│  │                              │ D. LLM critique  │                   │ │
│  │                              └──────────────────┘                   │ │
│  └──────────────────────────────────────────────────────────────────────┘ │
│                              │                                          │
│                              ▼                                          │
│  ┌──────────────────────────────────────────────────────────────────────┐ │
│  │  4. OUTPUT                                                         │ │
│  │  approved Lua code ──▶ user                                         │ │
│  └──────────────────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────┘

## State Machine

```
  GENERATING_PLAN
         │
         ▼
  AWAITING_PLAN_CONFIRMATION
         │ "Подтвердить"
         ▼
  GENERATING_CODE ──────────────┐
         │                  │
         ▼                  │
  AWAITING_CODE_APPROVAL    │ (feedback)
         │                  │
  "Подтвердить" ◀──────────┘
         │
         ▼
       DONE
```

## Flow Details

```
USER ──[task]──▶┐
                │
                ▼
         ┌──────────────┐
         │ NEW SESSION│
         └──────────────┘
                │
                ▼
         ┌──────────────┐
         │ Parse task  │ ◀── json_input_parser.py
         │ extract    │
         │ context   │
         └──────────────┘
                │
                ▼
         ┌──────────────┐
         │ Generate  │ ��── pipeline.py
         │ plan     │     _generate_plan()
         └──────────────┘
                │
         "Подтвердить"? ────NO───▶ REVISION LOOP
                │
               YES
                │
                ▼
         ┌──────────────┐
         │ Build RAG  │ ◀── rag_func.py
         │ context    │     build_rag_context()
         └──────────────┘
                │
                ▼
         ┌──────────────┐
         │ Generate   │ ◀── pipeline.py
         │ code      │     _generate_code()
         └──────────────┘
                │
                ▼
         ┌──────────────┐
         │ Sandbox    │ ◀── sandbox_client.py
         │ validate   │     send_code_for_validation()
         └──────────────┘
                │
           FAILED │ (CODE_RETRIES_SANDBOX = 20)
                ├────────────┐
                │           │
        ◀──────────┘         │
                │           ▼
         ┌──────────────┐    │
         │ FIX + RETRY  │◀───┘
         │ (feedback)  │
         └──────────────┘
                │
                ▼
         ┌──────────────┐
         │ Ollama     │ ◀── pipeline.py
         │ critique  │     _critique_code()
         └──────────────┘
                │
           FAILED │ (feedback)
                ├────────────┐
                │           │
        ◀──────────┘         │
                │           ▼
         ┌──────────────┐    │
         │ FIX + RETRY  │◀───┘
         │ (feedback)  │
         └──────────────┘
                │
                ▼
         ┌──────────────┐
         │   DONE     │
         │ (APPROVAL) │
         └──────────────┘
```

## Components

| Component | File | Purpose |
|----------|------|---------|
| FastAPI | `main.py` | HTTP endpoint, state machine |
| Pipeline | `pipeline.py` | LLM orchestration |
| OllamaClient | `ollama_client.py` | Ollama API calls |
| SandboxClient | `sandbox_client.py` | Sandbox validation |
| RAG | `rag_func.py` | Context from Qdrant |
| Parser | `json_input_parser.py` | Task parsing |

## Environment

```yaml
OLLAMA_URL:        http://ollama:11434
SANDBOX_SERVICE:  http://sandbox-service:6778
QDRANT_URL:       http://qdrant:6333
MODEL:           qwen2.5-coder:7b
RETRIES:          20 (sandbox), 2 (model)
```