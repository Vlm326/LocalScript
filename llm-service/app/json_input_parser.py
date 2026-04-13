import json
import re
from typing import Any, Dict, Optional
from typing import Tuple

from pydantic import BaseModel, Field



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

    context = _validate_context(obj)

    # вырезаем JSON из текста
    before = cleaned[:start]
    after = cleaned[start + end:]

    task_clean = (before + after).strip()

    if not task_clean:
        raise ParseError("Task text is empty after removing JSON")

    return task_clean, context

def _strip_code_fences(text: str) -> str:
    text = text.strip()
    if text.startswith("```"):
        text = re.sub(r"^```(?:json|python|text)?\s*", "", text, flags=re.IGNORECASE)
        text = re.sub(r"\s*```$", "", text)
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


def _validate_context(context: Any) -> Dict[str, Any]:
    if not isinstance(context, dict):
        raise ParseError("context must be an object")

    wf = context.get("wf")
    if not isinstance(wf, dict):
        raise ParseError("context.wf is required and must be an object")

    vars_ = wf.get("vars")
    if not isinstance(vars_, dict):
        raise ParseError("context.wf.vars is required and must be an object")

    return context
