import httpx
from qdrant_client import AsyncQdrantClient

from config import EMBEDDINGS_URL, EMBEDDING_MODEL, QDRANT_COLLECTION, QDRANT_URL, LIMIT_FOR_RAG_DOCS


async def connect_qdrant_client():
    return AsyncQdrantClient(url=QDRANT_URL)


async def get_emb_from_prompt(prompt: str, ip: str, http_client: httpx.AsyncClient):
    response = await http_client.post(
        url=ip,
        json={"input": prompt, "model": "bge-m3"},
    )
    response.raise_for_status()
    return response.json()["data"][0]["embedding"]


def _payload_to_text(payload):
    lines = []
    description = (payload.get("description") or "").strip()
    checklist = (payload.get("validation_checklist") or "").strip()

    if description:
        lines.append(f"Конструкция: {description}")
    if checklist:
        lines.append(f"Проверить: {checklist}")
    return "\n".join(lines)


def _doc_id(payload, fallback: int) -> str:
    value = payload.get("id") or payload.get("_id") or payload.get("path")
    return str(value) if value is not None else f"doc-{fallback}"


async def get_from_rag(
    qdrant_client: AsyncQdrantClient, query_vector, limit: int = LIMIT_FOR_RAG_DOCS
):
    sim_points = await qdrant_client.search(
        collection_name=QDRANT_COLLECTION,
        query_vector=query_vector,
        limit=limit,
    )

    if not sim_points:
        return ""

    point = sim_points[0]
    payload = point.payload or {}
    return _payload_to_text(payload)


def normalize_plan_text(plan: str) -> str:
    return "\n".join(line.rstrip() for line in plan.strip().splitlines()).strip()


def split_plan_into_chunks(plan: str) -> list[str]:
    text = normalize_plan_text(plan)
    if not text:
        return []

    return [
        " ".join(line.strip() for line in chunk.splitlines() if line.strip())
        for chunk in text.split("$:")
        if chunk.strip()
    ]


async def search_rag_for_chunk(chunk, qdrant_client, http_client, limit: int = LIMIT_FOR_RAG_DOCS):
    embedding = await get_emb_from_prompt(chunk, EMBEDDINGS_URL, http_client)
    sim_points = await qdrant_client.search(
        collection_name=QDRANT_COLLECTION,
        query_vector=embedding,
        limit=limit,
    )

    for index, point in enumerate(sim_points):
        payload = point.payload or {}
        text = _payload_to_text(payload)
        if text:
            return _doc_id(payload, index), text
    return ""


async def build_rag_context(plan: str, limit_per_chunk: int = LIMIT_FOR_RAG_DOCS) -> str:
    chunks = split_plan_into_chunks(plan)
    if not chunks:
        return ""

    qdrant_client = None
    try:
        qdrant_client = await connect_qdrant_client()
        timeout = httpx.Timeout(connect=5.0, read=15.0, write=5.0, pool=5.0)
        async with httpx.AsyncClient(timeout=timeout) as http_client:
            seen = set()
            blocks = []
            for chunk in chunks:
                doc_id, text = await search_rag_for_chunk(
                    chunk=chunk,
                    qdrant_client=qdrant_client,
                    http_client=http_client,
                    limit=limit_per_chunk,
                )
                if not text or doc_id in seen:
                    continue
                seen.add(doc_id)
                blocks.append(
                    "\n".join(
                        [
                            f"Matched plan chunk: {chunk}",
                            text,
                        ]
                    )
                )
    except Exception:
        return ""
    finally:
        if qdrant_client is not None:
            try:
                await qdrant_client.close()
            except Exception:
                pass

    return "\n\n".join(blocks)
