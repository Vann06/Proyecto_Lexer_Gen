"""Los scripts de demostración de workspace/ (uno por entrega).

Se usan en la presentación, así que cada uno tiene que hacer exactamente lo
que promete su encabezado:
- los de ejemplo: cero problemas al ANALIZAR y al EJECUTAR, y se pueden
  ejecutar **dos veces seguidas** (empiezan con DROP/CREATE DATABASE);
- los de errores: cada línea marcada `-- CÓDIGO` da ese error, en esa línea.
  (entrega3_errores_semanticos.sql lo verifica test_semantic.py.)
"""

import re
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

import api.main as main

WORKSPACE = Path(__file__).resolve().parents[2] / "workspace"
EJEMPLOS = ["demo.sql", "entrega1_lexer_parser.sql", "entrega2_ast.sql",
            "entrega4_create_drop_insert.sql", "entrega5_select_update_delete.sql",
            "entrega6_join_agregados.sql", "entrega7_indices_subconsultas.sql",
            "entrega8_integracion.sql"]


@pytest.fixture
def client(tmp_path, monkeypatch):
    monkeypatch.setattr(main, "DATA_DIR", tmp_path / "data")
    return TestClient(main.app)


def test_hay_un_script_por_entrega():
    nombres = {p.name for p in WORKSPACE.glob("entrega*.sql")}
    for n in range(1, 9):
        assert any(name.startswith(f"entrega{n}_") for name in nombres), f"falta el script de la entrega {n}"


@pytest.mark.parametrize("nombre", EJEMPLOS)
def test_script_de_ejemplo_sin_problemas_y_repetible(client, nombre):
    text = (WORKSPACE / nombre).read_text(encoding="utf-8")
    analisis = client.post("/api/sql/parse", json={"script": text}).json()
    assert analisis["problems"] == [], analisis["problems"]
    for vez in ("primera", "segunda"):
        resp = client.post("/api/sql/run", json={"script": text}).json()
        assert resp["ok"], (vez, resp["problems"])
        assert all(s["result"] is not None for s in resp["statements"]), vez
        assert len(resp["statements"]) == len(analisis["statements"]), vez


def test_entrega1_errores_da_el_codigo_de_cada_linea():
    from dbms.frontend import parse
    text = (WORKSPACE / "entrega1_errores.sql").read_text(encoding="utf-8")
    lines = text.splitlines()
    expected = {}
    for i, line in enumerate(lines, start=1):
        if m := re.search(r";\s+-- ((?:LEX|SYN)\d{3})", line):
            expected[i] = m.group(1)
        elif m := re.search(r"La línea siguiente debe dar ((?:LEX|SYN)\d{3})", line):
            expected[i + 1] = m.group(1)
    assert len(expected) == 7
    first_per_line = {}
    for p in sorted(parse(text).problems, key=lambda p: (p.line, p.col)):
        first_per_line.setdefault(p.line, p.code)
    assert first_per_line == expected
