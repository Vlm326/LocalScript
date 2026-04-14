import httpx
from qdrant_client import AsyncQdrantClient
from config import QDRANT_URL

async def connect_qdrant_client():
    return AsyncQdrantClient(url=QDRANT_URL)

async def get_emb_from_prompt(prompt: str, ip: str, http_client: httpx.AsyncClient):
    response = await http_client.post(
        url=ip,
        json={"input": prompt, "model": "bge-m3"},
    )
    response.raise_for_status()
    return response.json()["data"][0]["embedding"]

async def get_from_rag(qdrant_client: AsyncQdrantClient, query_vector: list[float], limit: int = 1):
    sim_points = await qdrant_client.search(
        collection_name="lua_patterns",
        query_vector=query_vector,
        limit=limit,
    )

    if not sim_points:
        return [None, ""]

    point = sim_points[0]
    payload = point.payload or {}

    text_of_doc = f"""
                    Конструкция: {payload.get('description', 'N/A')}
                    Проверить: {payload.get('validation_checklist', 'N/A')}
                    """
    
    return [payload.get('id'), text_of_doc]