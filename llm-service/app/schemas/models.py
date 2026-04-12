from enum import Enum
from typing import List, Optional

from pydantic import BaseModel, Field


class StatusTag(str, Enum):
    ok = "ok"
    syntax_error = "syntax_error"
    safety_error = "safety_error"
    runtime_error = "runtime_error"
    timeout = "timeout"


class ErrorKind(str, Enum):
    syntax_error = "syntax_error"
    safety_error = "safety_error"
    runtime_error = "runtime_error"
    timeout = "timeout"
    memory_limit = "memory_limit"
    stack_overflow = "stack_overflow"
    forbidden_access = "forbidden_access"
    unknown = "unknown"


class StructuredError(BaseModel):
    kind: ErrorKind
    message: str
    line: Optional[int] = None
    raw: str
    snippet: Optional[str] = None


class AstAnalysis(BaseModel):
    function_calls: List[str]
    has_dangerous_patterns: bool
    has_forbidden_calls: bool


class ExecutionStats(BaseModel):
    memory_used_bytes: Optional[int] = None
    execution_time_ms: Optional[int] = None


class SandboxRequest(BaseModel):
    code: str
    execute: Optional[bool] = False
    timeout: Optional[int] = Field(default=2, ge=1, le=10)


class SandboxResponse(BaseModel):
    status: StatusTag
    source_code: str
    output: Optional[str] = None
    logs: List[str] = Field(default_factory=list)
    warnings: List[str] = Field(default_factory=list)
    error_detail: Optional[StructuredError] = None
    ast_analysis: Optional[AstAnalysis] = None
    execution_stats: Optional[ExecutionStats] = None

    @property
    def is_ok(self) -> bool:
        return self.status == StatusTag.ok

    @property
    def error_summary(self) -> str:
        if self.error_detail:
            return self.error_detail.message
        if self.logs:
            return "; ".join(
                l for l in self.logs
                if l.startswith("[error]") or l.startswith("[fatal]")
            )
        return "unknown error"


class GenerateRequest(BaseModel):
    task: str
    execute: Optional[bool] = False
    timeout: Optional[int] = 2
    rag_data: Optional[str] = ""


class IterationRecord(BaseModel):
    attempt: int
    code_before: str
    feedback: str
    code_after: Optional[str] = None


class GenerateResponse(BaseModel):
    plan: str
    code: str
    sandbox_result: Optional[SandboxResponse] = None
    iterations: List[IterationRecord] = Field(default_factory=list)
    status: str
    clarification_question: Optional[str] = None


class ValidateRequest(BaseModel):
    code: str
    execute: Optional[bool] = True
    timeout: Optional[int] = 2


class ValidateResponse(BaseModel):
    sandbox_result: SandboxResponse
    code: str


class HealthResponse(BaseModel):
    status: str = "ok"
    ollama_model: Optional[str] = None
    sandbox_service_url: Optional[str] = None
