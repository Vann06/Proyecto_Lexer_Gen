"""API: endpoints de workspace y /api/sql/parse."""

import pytest
from fastapi.testclient import TestClient

import api.main as main


@pytest.fixture
def client(tmp_path, monkeypatch):
    monkeypatch.setattr(main, "WORKSPACE_DIR", tmp_path)
    monkeypatch.setattr(main, "DATA_DIR", tmp_path / "data")
    return TestClient(main.app)


def test_health(client):
    assert client.get("/health").json()["status"] == "ok"


def test_workspace_roundtrip(client):
    assert client.put("/api/workspace/demo.sql", content="SELECT 1;").status_code == 200
    assert client.get("/api/workspace/demo.sql").text == "SELECT 1;"
    assert client.get("/api/workspace").json() == {"files": [{"name": "demo.sql", "kind": "sql"}]}


@pytest.mark.parametrize("name", ["..secret.sql", "x.exe", "a%2Fb.sql"])
def test_workspace_rechaza_nombres_invalidos(client, name):
    assert client.get(f"/api/workspace/{name}").status_code in (400, 404)


def test_workspace_inexistente(client):
    assert client.get("/api/workspace/nada.sql").status_code == 404


def test_sql_parse_valido(client):
    data = client.post("/api/sql/parse", json={"script": "CREATE TABLE t (a INT);\nSELECT * FROM t;"}).json()
    assert data["ok"] and data["problems"] == []
    assert [s["kind"] for s in data["statements"]] == ["CreateTable", "Select"]
    assert data["statements"][1]["line"] == 2
    assert data["tokens"][0]["kind"] == "CREATE"
    assert data["parse_tree_dot"].startswith("digraph")
    assert data["ast_dot"].startswith("digraph AST")


def test_sql_parse_con_errores(client):
    data = client.post("/api/sql/parse", json={"script": "SELECT FROM t;"}).json()
    assert not data["ok"] and data["statements"] == []
    assert data["ast_dot"] == ""
    assert data["problems"][0]["code"] == "SYN002"
    assert data["problems"][0]["level"] == "err"
    assert data["parse_tree_dot"]   # el árbol parcial se devuelve igual


def test_sql_parse_con_errores_semanticos(client, tmp_path):
    data = client.post("/api/sql/parse", json={"script": "SELECT nombre\nFROM alumnos;"}).json()
    assert data["ok"]                       # la sintaxis está bien
    assert [(p["code"], p["line"], p["col"]) for p in data["problems"]] == [("SEM001", 2, 6)]
    assert data["ast_dot"]                  # el AST se dibuja igual
    assert (tmp_path / "data" / "principal" / "metadata.txt").is_file()


def test_sql_parse_base_inexistente(client):
    data = client.post("/api/sql/parse", json={"script": "SELECT 1;", "database": "nada"}).json()
    assert [p["code"] for p in data["problems"]] == ["EXE000"]


def test_grammar(client):
    text = client.get("/api/grammar").text
    assert text.lstrip().startswith("//") and "grammar SQL;" in text
