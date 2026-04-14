from __future__ import annotations

from typing import List


def parse_dollar_blocks(text: str) -> List[str]:
    """
    Извлекает блоки текста, начинающиеся с маркера ``$:``.

    Правила:
    - каждая строка с ``$:`` начинает новый блок;
    - последующие непустые строки без нового ``$:`` считаются продолжением текущего блока;
    - пустая строка завершает текущий блок;
    - лишние пробелы по краям игнорируются.
    """
    blocks: List[str] = []
    current_parts: List[str] = []

    for raw_line in text.splitlines():
        line = raw_line.strip()

        if line.startswith("$:"):
            _flush_block(current_parts, blocks)
            content = line[2:].strip()
            if content:
                current_parts.append(content)
            continue

        if not line:
            _flush_block(current_parts, blocks)
            continue

        if current_parts:
            current_parts.append(line)

    _flush_block(current_parts, blocks)
    return blocks

def _flush_block(current_parts: List[str], blocks: List[str]) -> None:
    if not current_parts:
        return

    block = " ".join(part.strip() for part in current_parts if part.strip()).strip()
    if block:
        blocks.append(block)
    current_parts.clear()


if __name__ == "__main__":
    sample = """
$: Извлекаем список email из переменной `emails`. В данном случае, это будет массив: ["user1@example.com", "user2@example.com", "user3@example.com"].

$: Получаем последний элемент этого массива. Поскольку в Lua индексация начинается с 1, последний элемент будет на позиции `#emails`.

$: Возвращаем этот элемент как результат функции или присваиваем его переменной для дальнейшего использования.
"""

    assert parse_dollar_blocks(sample) == [
        'Извлекаем список email из переменной `emails`. В данном случае, это будет массив: ["user1@example.com", "user2@example.com", "user3@example.com"].',
        "Получаем последний элемент этого массива. Поскольку в Lua индексация начинается с 1, последний элемент будет на позиции `#emails`.",
        "Возвращаем этот элемент как результат функции или присваиваем его переменной для дальнейшего использования.",
    ]
