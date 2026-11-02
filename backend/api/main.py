"""API HTTP del DBMS.

Endpoints del workspace (los archivos .sql que ve el IDE), /health y los de SQL:

    POST /api/sql/parse   {script, database?}  -> tokens, árbol DOT, AST DOT,
                          problemas (léxicos, sintácticos y semánticos), sentencias
    POST /api/sql/run     {script, database?}  -> por sentencia: problemas y
                          resultado; se detiene en la primera con error
    GET  /api/catalog?database=  -> bases, tablas (columnas, restricciones,
                          filas) e índices de una base
    GET  /api/grammar               -> texto de grammar/SQL.g4 (el IDE lo muestra)

Correr desde backend/:  uvicorn api.main:app --port 8080 --reload
"""

from __future__ import annotations

import os
from pathlib import Path

from fastapi import FastAPI, HTTPException, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import PlainTextResponse
from pydantic import BaseModel

from dbms import ast
from dbms.ast_dot import ast_to_dot
from dbms.builder import build
from dbms.executor import execute
from dbms.frontend import Problem, parse, tokens_to_json, tree_to_dot
from dbms.result import ExecutionError, Result
from dbms.semantic import Analyzer, check_script
from dbms.storage import DEFAULT_DATABASE, DataDir, StorageError

WORKSPACE_DIR = Path(os.environ.get("DBMS_WORKSPACE", Path(__file__).resolve().parents[2] / "workspace"))
DATA_DIR = Path(os.environ.get("DBMS_DATA", Path(__file__).resolve().parents[2] / "data"))
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
    database: str = DEFAULT_DATABASE


def _semantic_problems(stmts, database: str) -> list[Problem]:
    """Análisis semántico del script contra el catálogo de `database`. Solo
    lee: no ejecuta nada ni modifica archivos (salvo crear la base principal
    vacía la primera vez)."""
    try:
        data = DataDir(DATA_DIR)
        db = data.open(database, create=database == DEFAULT_DATABASE)
        return check_script(stmts, db.load_catalog(), data, database)
    except StorageError as e:
        return [Problem("err", "EXE000", f"No se pudo leer la base de datos: {e}")]


@app.post("/api/sql/parse")
def sql_parse(req: ScriptRequest):
    """Análisis léxico, sintáctico y semántico. No ejecuta sentencias."""
    result = parse(req.script)
    stmts = build(result) if result.ok else []
    # La semántica necesita el AST: con errores de sintaxis no corre.
    semantic = _semantic_problems(stmts, req.database) if result.ok else []
    return {
        "ok": result.ok,
        "problems": [p.to_dict() for p in result.problems + semantic],
        "tokens": tokens_to_json(result),
        "parse_tree_dot": tree_to_dot(result),
        # Sin AST si hubo errores de sintaxis (ver dbms.builder.build).
        "ast_dot": ast_to_dot(stmts) if result.ok else "",
        "statements": [{"kind": s.kind, "sql": s.sql, "line": s.pos.line, "col": s.pos.col}
                       for s in stmts],
    }


def _run_database_stmt(s, data: DataDir, current: str) -> tuple[Result, str]:
    """Sentencias sobre bases de datos: las resuelve la API porque necesitan
    el DataDir, no una sola base. Devuelve el resultado y la base activa."""
    if isinstance(s, ast.CreateDatabase):
        if data.exists(s.name):
            return Result(message=f"La base de datos '{s.name}' ya existe"), current
        data.create(s.name)
        return Result(message=f"Base de datos '{s.name}' creada"), current
    if isinstance(s, ast.DropDatabase):
        if not data.exists(s.name):
            return Result(message=f"La base de datos '{s.name}' no existe"), current
        data.drop(s.name)
        # Si se borró la base activa se vuelve a la principal.
        return Result(message=f"Base de datos '{s.name}' eliminada"), DEFAULT_DATABASE if s.name == current else current
    if isinstance(s, ast.UseDatabase):
        return Result(message=f"Usando la base de datos '{s.name}'"), s.name
    return Result(["base"], [[n] for n in data.databases()]), current   # ShowDatabases


_DATABASE_STMTS = (ast.CreateDatabase, ast.DropDatabase, ast.UseDatabase, ast.ShowDatabases)


@app.post("/api/sql/run")
def sql_run(req: ScriptRequest):
    """Ejecuta el script sentencia por sentencia: semántica contra el catálogo
    actual (ya con los efectos de las anteriores) y luego ejecución. Se
    detiene en la primera sentencia con error; las anteriores quedan hechas."""
    parsed = parse(req.script)
    if not parsed.ok:
        return {"ok": False, "database": req.database,
                "problems": [p.to_dict() for p in parsed.problems], "statements": []}

    data, current = DataDir(DATA_DIR), req.database
    problems, statements = list(parsed.problems), []
    for stmt in build(parsed):
        entry = {"kind": stmt.kind, "sql": stmt.sql, "line": stmt.pos.line, "col": stmt.pos.col,
                 "problems": [], "result": None}
        statements.append(entry)
        try:
            db = data.open(current, create=current == DEFAULT_DATABASE)
            found = Analyzer(db.load_catalog(), data, current).check(stmt)
            if not any(p.level == "err" for p in found):
                if isinstance(stmt.body, _DATABASE_STMTS):
                    result, current = _run_database_stmt(stmt.body, data, current)
                else:
                    result = execute(stmt, db)
                entry["result"] = result.to_dict()
        except ExecutionError as e:
            found = [e.to_problem()]
        except StorageError as e:
            found = [Problem("err", "EXE000", f"Error de almacenamiento: {e}", stmt.pos.line, stmt.pos.col)]
        entry["problems"] = [p.to_dict() for p in found]
        problems += found
        if entry["result"] is None:
            break
    return {"ok": not any(p.level == "err" for p in problems), "database": current,
            "problems": [p.to_dict() for p in problems], "statements": statements}


@app.get("/api/catalog")
def catalog(database: str = DEFAULT_DATABASE):
    data = DataDir(DATA_DIR)
    try:
        db = data.open(database, create=database == DEFAULT_DATABASE)
        cat = db.load_catalog()
        tables = [{
            "name": t.name,
            "columns": [{"name": c.name, "type": str(c.type), "not_null": c.not_null} for c in t.columns],
            "constraints": [{"kind": k.kind, "name": k.name, "columns": k.columns,
                             "ref_table": k.ref_table, "ref_columns": k.ref_columns, "check": k.check}
                            for k in t.constraints],
            "rows": len(db.read_rows(t)),
        } for t in cat.tables.values()]
    except StorageError as e:
        raise HTTPException(status_code=404, detail=str(e))
    return {"database": database, "databases": data.databases(), "tables": tables,
            "indexes": [{"name": i.name, "table": i.table, "columns": i.columns,
                         "unique": i.unique, "implicit": i.implicit} for i in cat.indexes.values()]}
