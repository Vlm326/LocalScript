from qdrant_client import QdrantClient
from config import QDRANT_URL
import requests

def get_emb_from_prompt(prompt, ip):
    response = requests.post(
        url=ip,
        json={
            "input": prompt,
            "model": "bge-m3",
        },
    )
    return response.json()["data"][0]["embedding"]

def get_from_rag(client, query, limit):
    '''Возвращает массив: [айди, текст документа]'''
    sim_points = client.query_points(collection_name = 'lua_patterns', query=query, limit=limit)
    point = sim_points[0]
    text_of_doc = f'''
                    Конструкция: {point.payload['description']}
                    Проверить: {point.payload['validation_checklist']}
                    '''
    
    return [point.payload['id', text_of_doc]]


