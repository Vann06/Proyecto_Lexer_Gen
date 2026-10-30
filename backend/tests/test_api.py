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


def test_analizar_y_ejecutar_en_base_activa(client):
    setup = client.post('/api/sql/run', json={'script':
        'CREATE DATABASE qa; USE qa; CREATE TABLE exclusiva (id INT); '
        'INSERT INTO exclusiva VALUES (1);'}).json()
    assert setup['ok']
    payload = {'script': 'SELECT * FROM exclusiva;', 'database': setup['database']}
    analyzed = client.post('/api/sql/parse', json=payload).json()
    assert analyzed['ok']
    assert not [p for p in analyzed['problems'] if p['level'] == 'err']
    executed = client.post('/api/sql/run', json=payload).json()
    assert executed['ok']
    assert executed['statements'][0]['result']['rows'] == [[1]]
    # La tabla solo existe en qa, por lo que omitir database sí sería un error.
    principal = client.post('/api/sql/parse', json={'script': payload['script']}).json()
    assert [p['code'] for p in principal['problems']] == ['SEM001']


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


SCRIPT = """CREATE TABLE estudiantes (id INT PRIMARY KEY, nombre VARCHAR(30), edad INT);
INSERT INTO estudiantes VALUES (1, 'Ana', 19), (2, 'Luis', 17);
SELECT nombre FROM estudiantes WHERE edad >= 18;"""


def test_sql_run_ejecuta_y_persiste(client, tmp_path):
    data = client.post("/api/sql/run", json={"script": SCRIPT}).json()
    assert data["ok"] and data["problems"] == []
    assert [s["kind"] for s in data["statements"]] == ["CreateTable", "Insert", "Select"]
    assert data["statements"][1]["result"]["message"] == "2 filas afectadas"
    assert data["statements"][2]["result"]["rows"] == [["Ana"]]
    assert (tmp_path / "data" / "principal" / "estudiantes.csv").is_file()


def test_sql_run_se_detiene_en_el_primer_error(client):
    script = "CREATE TABLE t (a INT PRIMARY KEY);\nINSERT INTO t VALUES (1), (1);\nSELECT * FROM t;"
    data = client.post("/api/sql/run", json={"script": script}).json()
    assert not data["ok"]
    assert len(data["statements"]) == 2                     # SELECT no se ejecutó
    assert data["statements"][1]["result"] is None
    assert [(p["code"], p["line"]) for p in data["problems"]] == [("EXE001", 2)]


def test_sql_run_error_semantico_no_ejecuta(client):
    data = client.post("/api/sql/run", json={"script": "SELECT x FROM nada;"}).json()
    assert [p["code"] for p in data["problems"]] == ["SEM001"]
    assert data["statements"][0]["result"] is None


def test_sql_run_error_de_sintaxis(client):
    data = client.post("/api/sql/run", json={"script": "SELECT FROM t;"}).json()
    assert not data["ok"] and data["statements"] == []
    assert data["problems"][0]["code"].startswith("SYN")


def test_sql_run_bases_de_datos(client):
    script = "CREATE DATABASE tienda;\nUSE tienda;\nCREATE TABLE c (id INT);\nSHOW DATABASES;"
    data = client.post("/api/sql/run", json={"script": script}).json()
    assert data["ok"] and data["database"] == "tienda"
    assert data["statements"][3]["result"]["rows"] == [["principal"], ["tienda"]]
    assert [t["name"] for t in client.get("/api/catalog?database=tienda").json()["tables"]] == ["c"]


def test_catalog(client):
    client.post("/api/sql/run", json={"script": SCRIPT + "\nCREATE INDEX idx_edad ON estudiantes (edad);"})
    data = client.get("/api/catalog").json()
    assert data["database"] == "principal" and data["databases"] == ["principal"]
    [t] = data["tables"]
    assert t["name"] == "estudiantes" and t["rows"] == 2
    assert [c["name"] for c in t["columns"]] == ["id", "nombre", "edad"]
    assert {i["name"] for i in data["indexes"]} >= {"idx_edad"}


def test_catalog_base_inexistente(client):
    assert client.get("/api/catalog?database=nada").status_code == 404
