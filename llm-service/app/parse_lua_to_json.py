import json
import re


def replace_indentation(line):
    match = re.match(r"^( +)", line)  # ищем все пробелы
    if not match:
        return line

    spaces_count = len(match.group(1))  # только первые пробелы
    tabs = spaces_count // 4
    remainder = spaces_count % 4

    new_prefix = ("\t" * tabs) + (" " * remainder)
    return new_prefix + line[spaces_count:]


def parse_lua_to_json(lua_code: str) -> str:
    """
    Преобразование Lua-кода в JSON-объект.
    """

    lines = lua_code.splitlines()
    processed_lines = [replace_indentation(line) for line in lines]

    normalized_code = "\n" + "\n".join(processed_lines)

    content = f"lua{{{normalized_code}\n}}lua"

    data = {"result": content}

    return json.dumps(data, ensure_ascii=False)
