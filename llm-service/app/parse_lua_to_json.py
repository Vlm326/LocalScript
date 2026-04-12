import json
import re

def parse_lua_to_json(lua_code: str) -> str:
    """
    Преобразования ответа модели в корретный код
    """
    
    def replace_spaces(match):
        spaces = match.group(0)
        if spaces.startswith('\n'):
            prefix = '\n'
            pure_spaces = spaces[1:]
        else:
            prefix = ''
            pure_spaces = spaces
            
        tab_count = len(pure_spaces) // 4
        remaining_spaces = ' ' * (len(pure_spaces) % 4)
        
        return prefix + ('\t' * tab_count) + remaining_spaces

    normalized_code = re.sub(r'(?:^|\n)( {4})+', replace_spaces, lua_code)
    
    content = f"lua{{{normalized_code}}}lua"
    
    data = {"result": content}
    return json.dumps(data, ensure_ascii=False)
