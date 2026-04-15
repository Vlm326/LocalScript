import json
import re
from typing import Any, Dict, Tuple



class ParseError(ValueError):
    pass


def extract_context_and_clean_task(text: str) -> Tuple[str, dict]:
    """
    Возвращает:
    - очищенный task (без JSON)
    - context (dict)
    """
    cleaned = _strip_code_fences(text)

    start = cleaned.find("{")
    if start == -1:
        raise ParseError("JSON context not found in task")

    decoder = json.JSONDecoder()

    try:
        obj, end = decoder.raw_decode(cleaned[start:])
    except json.JSONDecodeError as e:
        raise ParseError(f"Invalid JSON: {e}") from e

    context = _normalize_and_validate_context(obj)

    # вырезаем JSON из текста
    before = cleaned[:start]
    after = cleaned[start + end :]

    task_clean = _strip_code_fences((before + after)).strip()

    if not task_clean:
        raise ParseError("Task text is empty after removing JSON")

    return task_clean, context


def _strip_code_fences(text: str) -> str:
    text = text.strip()
    # Remove surrounding or standalone code fences (best-effort).
    if "```" in text:
        # Strip common opening fences at the beginning.
        text = re.sub(r"^```(?:json|python|text)?\s*", "", text, flags=re.IGNORECASE)
        # Strip common closing fence at the end.
        text = re.sub(r"\s*```$", "", text)
        # Drop standalone fence lines left in the middle after JSON removal.
        text = re.sub(r"(?m)^\s*```(?:json|python|text)?\s*$", "", text, flags=re.IGNORECASE)
    return text.strip()


def _extract_first_json_object(text: str) -> Dict[str, Any]:
    text = _strip_code_fences(text)

    start = text.find("{")
    if start == -1:
        raise ParseError("JSON object not found")

    decoder = json.JSONDecoder()
    try:
        obj, _ = decoder.raw_decode(text[start:])
    except json.JSONDecodeError as e:
        raise ParseError(f"Invalid JSON: {e}") from e

    if not isinstance(obj, dict):
        raise ParseError("Top-level JSON must be an object")

    return obj


def _normalize_and_validate_context(context: Any) -> Dict[str, Any]:
    """
    Поддерживаем два формата входного контекста:

    A) {"wf": {"vars": {...}, "initVariables": {...}}}
    B) {"vars": {...}, "initVariables": {...}}

    Возвращаем нормализованный формат A.
    """
    if not isinstance(context, dict):
        raise ParseError("context must be an object")

    wf = context.get("wf")
    if wf is None:
        wf = {
            "vars": context.get("vars", {}),
            "initVariables": context.get("initVariables", {}),
        }
        context = {"wf": wf}

    if not isinstance(wf, dict):
        raise ParseError("context.wf must be an object")

    vars_ = wf.get("vars")
    if not isinstance(vars_, dict):
        raise ParseError("context.wf.vars is required and must be an object")

    init_vars = wf.get("initVariables", {})
    if init_vars is None:
        init_vars = {}
    if not isinstance(init_vars, dict):
        raise ParseError("context.wf.initVariables must be an object")

    wf.setdefault("initVariables", init_vars)
    context["wf"] = wf
    return context
