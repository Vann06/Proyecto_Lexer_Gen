"""API HTTP del DBMS.

Mismos endpoints de workspace y /health que tenía el servidor Rust anterior
(el IDE los usa sin cambios), más los de SQL:

    POST /api/sql/parse   {script}  -> tokens, árbol DOT, problemas, sentencias
    GET  /api/grammar               -> texto de grammar/SQL.g4 (el IDE lo muestra)
    (fase 8: /api/sql/run y /api/catalog)

Correr desde backend/:  uvicorn api.main:app --port 8080 --reload
"""

from __future__ import annotations

import os
from pathlib import Path

from fastapi import FastAPI, HTTPException, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import PlainTextResponse
from pydantic import BaseModel

from dbms.builder import build
from dbms.frontend import parse, tokens_to_json, tree_to_dot

WORKSPACE_DIR = Path(os.environ.get("DBMS_WORKSPACE", Path(__file__).resolve().parents[2] / "workspace"))
ALLOWED_EXTENSIONS = (".sql", ".txt")
GRAMMAR_FILE = Path(__file__).resolve().parents[1] / "grammar" / "SQL.g4"

app = FastAPI(title="DBMS SQL API")
app.add_middleware(CORSMiddleware, allow_origins=["*"], allow_methods=["*"], allow_headers=["*"])


# ───────────────────────────── Salud y workspace ─────────────────────────────

@app.get("/health")
def health():
    return {"status": "ok", "service": "dbms-api"}


def _workspace_path(name: str) -> Path:
    """Solo nombres planos con extensión permitida: nada de rutas ni `..`."""
    if "/" in name or "\\" in name or ".." in name or not name.endswith(ALLOWED_EXTENSIONS):
        raise HTTPException(status_code=400, detail="invalid filename")
    return WORKSPACE_DIR / name


@app.get("/api/workspace")
def workspace_list():
    WORKSPACE_DIR.mkdir(parents=True, exist_ok=True)
    files = [{"name": p.name, "kind": "sql" if p.suffix == ".sql" else "txt"}
             for p in WORKSPACE_DIR.iterdir() if p.is_file() and p.name.endswith(ALLOWED_EXTENSIONS)]
    return {"files": sorted(files, key=lambda f: f["name"])}


@app.get("/api/workspace/{name}", response_class=PlainTextResponse)
def workspace_read(name: str):
    path = _workspace_path(name)
    if not path.is_file():
        raise HTTPException(status_code=404, detail="not found")
    return path.read_text(encoding="utf-8")


@app.put("/api/workspace/{name}")
async def workspace_write(name: str, request: Request):
    path = _workspace_path(name)
    WORKSPACE_DIR.mkdir(parents=True, exist_ok=True)
    path.write_bytes(await request.body())
    return {"ok": True}


# ───────────────────────────── SQL ─────────────────────────────

@app.get("/api/grammar", response_class=PlainTextResponse)
def grammar():
    return GRAMMAR_FILE.read_text(encoding="utf-8")


class ScriptRequest(BaseModel):
    script: str


@app.post("/api/sql/parse")
def sql_parse(req: ScriptRequest):
    """Solo análisis léxico y sintáctico: no toca ninguna base de datos."""
    result = parse(req.script)
    statements = []
    if result.ok:
        statements = [{"kind": s.kind, "sql": s.sql, "line": s.pos.line, "col": s.pos.col}
                      for s in build(result)]
    return {
        "ok": result.ok,
        "problems": [p.to_dict() for p in result.problems],
        "tokens": tokens_to_json(result),
        "parse_tree_dot": tree_to_dot(result),
        "statements": statements,
    }
