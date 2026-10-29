"""Aceptación de D1, D2, D4 (índices) y E contra los criterios de PLAN.md.

Todo pasa por POST /api/sql/run, igual que el botón EJECUTAR del IDE, sobre
una base temporal. Correr desde backend/:  PYTHONPATH=. python -m pytest <este archivo>
"""

import random

import pytest
from fastapi.testclient import TestClient

import api.main as main
from dbms import index
from dbms.storage import DataDir


@pytest.fixture
def api(tmp_path, monkeypatch):
    monkeypatch.setattr(main, "DATA_DIR", tmp_path / "data")
    client = TestClient(main.app)

    def run(script, database="principal"):
        return client.post("/api/sql/run", json={"script": script, "database": database}).json()

    run.client = client
    run.data = tmp_path / "data"
    return run


def ok(resp):
    assert resp["ok"], resp["problems"]
    return resp


def last(resp):
    return resp["statements"][-1]["result"]


def rows(resp):
    return last(ok(resp))["rows"]


SCHEMA = """
CREATE TABLE estudiantes (id INT PRIMARY KEY, nombre VARCHAR(20) NOT NULL, carne VARCHAR(5) UNIQUE,
                          edad INT, promedio FLOAT, activo BOOLEAN, ingreso DATE);
INSERT INTO estudiantes VALUES
  (1, 'Ana',   'A1', 20, 88.5, TRUE,  '2023-01-15'),
  (2, 'Luis',  'A2', 17, 75.0, TRUE,  '2024-01-10'),
  (3, 'Marta', NULL, NULL, 90.0, FALSE, '2022-07-01'),
  (4, 'Pedro', NULL, 22, NULL, TRUE,  NULL);
"""


# ───────────────────────────── D1 ─────────────────────────────

def test_d1_create_crea_archivos_y_catalogo(api):
    ok(api(SCHEMA))
    db = api.data / "principal"
    assert "TABLE estudiantes" in (db / "metadata.txt").read_text(encoding="utf-8")
    lines = (db / "estudiantes.csv").read_text(encoding="utf-8").splitlines()
    assert lines[0] == "_rowid,id,nombre,carne,edad,promedio,activo,ingreso"
    assert len(lines) == 5
    assert ok(api("CREATE TABLE IF NOT EXISTS estudiantes (x INT);"))


def test_d1_drop_borra_todo(api):
    ok(api(SCHEMA + "CREATE INDEX idx_edad ON estudiantes (edad);"))
    db = api.data / "principal"
    ok(api("DROP TABLE estudiantes;"))
    assert not list(db.glob("estudiantes*")) and not list(db.glob("*.idx"))
    assert "estudiantes" not in (db / "metadata.txt").read_text(encoding="utf-8")
    ok(api("DROP TABLE IF EXISTS estudiantes;"))


def test_d1_insert_null_en_omitidas_y_rowid_no_se_reutiliza(api):
    ok(api(SCHEMA))
    ok(api("INSERT INTO estudiantes (id, nombre) VALUES (5, 'Eva');"))
    assert rows(api("SELECT carne, edad, ingreso FROM estudiantes WHERE id = 5;")) == [[None, None, None]]
    ok(api("DELETE FROM estudiantes WHERE id = 5; INSERT INTO estudiantes (id, nombre) VALUES (6, 'Leo');"))
    csv = (api.data / "principal" / "estudiantes.csv").read_text(encoding="utf-8")
    assert csv.splitlines()[-1].startswith("6,6,Leo")       # _rowid 6: el 5 no se reutiliza


@pytest.mark.parametrize("sql, code", [
    ("INSERT INTO estudiantes (id, nombre) VALUES (1, 'Otra');", "EXE001"),           # PK contra fila existente
    ("INSERT INTO estudiantes (id, nombre) VALUES (9, 'X'), (9, 'Y');", "EXE001"),    # PK dentro del mismo INSERT
    ("INSERT INTO estudiantes (id, nombre, carne) VALUES (9, 'X', 'A1');", "EXE001"),  # UNIQUE
    ("UPDATE estudiantes SET id = 1 WHERE id = 2;", "EXE001"),                         # UPDATE que duplica la PK
    ("UPDATE estudiantes SET nombre = nombre || NULL WHERE id = 1;", "EXE002"),        # NULL calculado en NOT NULL
])
def test_d1_restricciones_y_archivo_intacto(api, sql, code):
    ok(api(SCHEMA))
    path = api.data / "principal" / "estudiantes.csv"
    before = path.read_text(encoding="utf-8")
    resp = api(sql)
    assert [p["code"] for p in resp["problems"]] == [code]
    assert path.read_text(encoding="utf-8") == before             # nada a medias


def test_d1_unique_admite_varios_null(api):
    ok(api(SCHEMA))
    ok(api("INSERT INTO estudiantes (id, nombre, carne) VALUES (7, 'X', NULL), (8, 'Y', NULL);"))


def test_d1_select_simple(api):
    ok(api(SCHEMA))
    r = last(ok(api("SELECT * FROM estudiantes;")))
    assert r["columns"] == ["id", "nombre", "carne", "edad", "promedio", "activo", "ingreso"]
    assert r["rows"][0] == [1, "Ana", "A1", 20, 88.5, True, "2023-01-15"]


# ───────────────────────────── D2 ─────────────────────────────

def test_d2_ejemplo_del_enunciado(api):
    ok(api(SCHEMA))
    assert rows(api("SELECT nombre FROM estudiantes WHERE edad >= 18;")) == [["Ana"], ["Pedro"]]


def test_d2_logica_de_tres_valores(api):
    ok(api(SCHEMA))
    q = "SELECT nombre FROM estudiantes WHERE {} ORDER BY id;"
    assert rows(api(q.format("NOT (edad >= 18)"))) == [["Luis"]]             # Marta (NULL) no entra
    assert rows(api(q.format("edad IS NULL"))) == [["Marta"]]
    assert rows(api(q.format("edad > 100 OR activo = FALSE"))) == [["Marta"]]
    assert rows(api(q.format("edad NOT IN (17, 20)"))) == [["Pedro"]]
    assert rows(api(q.format("ingreso >= '2023-01-01'"))) == [["Ana"], ["Luis"]]


def test_d2_order_by(api):
    ok(api(SCHEMA))
    assert rows(api("SELECT nombre FROM estudiantes ORDER BY edad;")) == [["Luis"], ["Ana"], ["Pedro"], ["Marta"]]
    assert rows(api("SELECT nombre FROM estudiantes ORDER BY edad DESC;")) == [["Pedro"], ["Ana"], ["Luis"], ["Marta"]]
    assert rows(api("SELECT nombre, promedio AS p FROM estudiantes ORDER BY activo DESC, p DESC;")) == [
        ["Ana", 88.5], ["Luis", 75.0], ["Pedro", None], ["Marta", 90.0]]


def test_d2_update_y_delete(api):
    ok(api(SCHEMA))
    r = last(ok(api("UPDATE estudiantes SET edad = edad + 1, promedio = 100 WHERE activo = TRUE;")))
    assert r["message"] == "3 filas afectadas"
    assert rows(api("SELECT edad, promedio FROM estudiantes WHERE id = 1;")) == [[21, 100.0]]
    csv = (api.data / "principal" / "estudiantes.csv").read_text(encoding="utf-8")
    assert ",100.0," in csv                                    # INT -> FLOAT al guardar
    # SET simultáneo: las dos expresiones ven la fila original.
    ok(api("UPDATE estudiantes SET edad = id, id = edad WHERE id = 1;"))
    assert rows(api("SELECT id, edad FROM estudiantes WHERE nombre = 'Ana';")) == [[21, 1]]
    assert last(ok(api("DELETE FROM estudiantes WHERE edad IS NULL;")))["message"] == "1 filas afectadas"
    assert len(rows(api("SELECT * FROM estudiantes;"))) == 3


# ───────────────────────────── D4: índices ─────────────────────────────

@pytest.mark.parametrize("where, used", [
    ("edad = 20", True), ("edad < 20", True), ("edad <= 20", True), ("edad > 20", True),
    ("edad >= 20", True), ("18 <= edad", True), ("edad >= 18 AND activo = TRUE", True),
    ("activo = TRUE", False), ("edad = 20 OR edad = 17", False),
])
def test_d4_plan_usa_el_indice(api, where, used):
    ok(api(SCHEMA + "CREATE INDEX idx_edad ON estudiantes (edad);"))
    plan = last(ok(api(f"SELECT nombre FROM estudiantes WHERE {where};")))["plan"]
    assert plan == ("índice idx_edad" if used else "recorrido completo")


def test_d4_indice_con_alias_y_fecha(api):
    ok(api(SCHEMA + "CREATE INDEX idx_ing ON estudiantes (ingreso); CREATE INDEX idx_edad ON estudiantes (edad);"))
    r = last(ok(api("SELECT e.nombre FROM estudiantes e WHERE e.edad = 20;")))
    assert (r["rows"], r["plan"]) == ([["Ana"]], "índice idx_edad")
    r = last(ok(api("SELECT nombre FROM estudiantes WHERE ingreso < '2023-06-01' ORDER BY id;")))
    assert (r["rows"], r["plan"]) == ([["Ana"], ["Marta"]], "índice idx_ing")


def test_d4_archivos_y_unique(api):
    ok(api(SCHEMA + "CREATE INDEX idx_edad ON estudiantes (edad);"))
    db = api.data / "principal"
    assert (db / "idx_edad.idx").is_file()
    ok(api("DROP INDEX idx_edad;"))
    assert not (db / "idx_edad.idx").exists()
    ok(api("INSERT INTO estudiantes (id, nombre, edad) VALUES (9, 'Dup', 20);"))
    resp = api("CREATE UNIQUE INDEX u_edad ON estudiantes (edad);")
    assert [p["code"] for p in resp["problems"]] == ["EXE001"]
    assert not (db / "u_edad.idx").exists()
    assert "u_edad" not in (db / "metadata.txt").read_text(encoding="utf-8")


def test_d4_indice_da_lo_mismo_que_el_recorrido(api):
    """Datos aleatorios con NULL: cada consulta con índice devuelve lo mismo
    que sin él, y tras INSERT/UPDATE/DELETE el .idx coincide con uno
    reconstruido desde cero."""
    rnd = random.Random(7)
    ok(api("CREATE TABLE t (id INT PRIMARY KEY, a INT, b FLOAT, c VARCHAR(3));"))
    values = ", ".join(f"({i}, {rnd.choice(['NULL', rnd.randint(-5, 15)])}, {rnd.choice(['NULL', round(rnd.uniform(0, 9), 1)])},"
                       f" {rnd.choice(['NULL', repr(rnd.choice('xyz') * rnd.randint(1, 3))])})" for i in range(200))
    ok(api(f"INSERT INTO t VALUES {values};"))
    queries = [f"SELECT id FROM t WHERE {col} {op} {lit} ORDER BY id;"
               for col, lits in (("a", [-3, 0, 7, 15, 99]), ("b", [0, 4.5, 9]), ("c", ["'x'", "'yy'", "'zzz'"]))
               for op in ("=", "<", "<=", ">", ">=") for lit in lits]
    queries += ["SELECT id FROM t WHERE a >= 2 AND a < 9 AND b > 3 ORDER BY id;",
                "SELECT id FROM t WHERE 5 > a AND c = 'xx' ORDER BY id;"]
    expected = [rows(api(q)) for q in queries]
    ok(api("CREATE INDEX ia ON t (a); CREATE INDEX ib ON t (b); CREATE INDEX ic ON t (c);"))
    for q, exp in zip(queries, expected):
        r = last(ok(api(q)))
        assert r["rows"] == exp, q
        assert r["plan"].startswith("índice"), q

    ok(api("UPDATE t SET a = a + 3 WHERE a < 4; DELETE FROM t WHERE b > 7; "
           "INSERT INTO t VALUES (500, 4, NULL, 'x'), (501, NULL, 2.5, NULL);"))
    db = DataDir(api.data).open("principal")
    catalog, table = db.load_catalog(), None
    table = catalog.table("t")
    current = db.read_rows(table)
    for name in ("ia", "ib", "ic", "pk_t"):
        idx = catalog.index(name)
        on_disk = (db.path / f"{name}.idx").read_text(encoding="utf-8")
        assert on_disk == index.build_text(idx, current), name
    ok(api("DROP INDEX ia; DROP INDEX ib; DROP INDEX ic;"))
    after = [rows(api(q)) for q in queries]
    ok(api("CREATE INDEX ia ON t (a); CREATE INDEX ib ON t (b); CREATE INDEX ic ON t (c);"))
    assert [rows(api(q)) for q in queries] == after


# ───────────────────────────── D3 ─────────────────────────────

CURSOS = SCHEMA + """
CREATE TABLE cursos (id INT PRIMARY KEY, nombre VARCHAR(20) NOT NULL, creditos INT);
CREATE TABLE inscripciones (estudiante_id INT, curso_id INT, nota FLOAT, PRIMARY KEY (estudiante_id, curso_id));
INSERT INTO cursos VALUES (1, 'Compiladores', 5), (2, 'Redes', 3), (3, 'Estadistica', 2);
INSERT INTO inscripciones VALUES (1, 1, 91), (1, 2, 55.5), (2, 2, 80), (3, 1, NULL), (4, 1, 70);
"""


def test_d3_conteos_y_promedios_por_curso(api):
    """Criterio de "hecho" de D3: estudiantes × inscripciones × cursos."""
    ok(api(CURSOS))
    r = last(ok(api("""
        SELECT c.nombre, COUNT(*) AS inscritos, COUNT(i.nota) AS con_nota, AVG(i.nota) AS promedio
        FROM estudiantes e
        INNER JOIN inscripciones i ON i.estudiante_id = e.id
        INNER JOIN cursos c ON c.id = i.curso_id
        GROUP BY c.nombre
        HAVING COUNT(*) >= 1
        ORDER BY inscritos DESC, c.nombre;""")))
    assert r["columns"] == ["nombre", "inscritos", "con_nota", "promedio"]
    assert r["rows"] == [["Compiladores", 3, 2, 80.5], ["Redes", 2, 2, 67.75]]
    assert rows(api("SELECT COUNT(*), MAX(nota), MIN(nota) FROM inscripciones;")) == [[5, 91.0, 55.5]]


# ───────────────────────────── D4: subconsultas ─────────────────────────────

def test_d4_subconsultas_del_enunciado(api):
    ok(api(CURSOS))
    q = {
        "IN": "SELECT nombre FROM estudiantes WHERE id IN (SELECT estudiante_id FROM inscripciones WHERE curso_id = 2) ORDER BY id;",
        "NOT IN": "SELECT nombre FROM estudiantes WHERE id NOT IN (SELECT estudiante_id FROM inscripciones);",
        "EXISTS": "SELECT e.nombre FROM estudiantes e WHERE EXISTS (SELECT 1 FROM inscripciones i WHERE i.estudiante_id = e.id AND i.nota >= 80) ORDER BY e.id;",
        "NOT EXISTS": "SELECT c.nombre FROM cursos c WHERE NOT EXISTS (SELECT 1 FROM inscripciones i WHERE i.curso_id = c.id);",
        "escalar": "SELECT nombre FROM estudiantes WHERE edad > (SELECT AVG(edad) FROM estudiantes) ORDER BY id;",
    }
    assert rows(api(q["IN"])) == [["Ana"], ["Luis"]]
    assert rows(api(q["NOT IN"])) == []                 # los cuatro tienen inscripción
    assert rows(api(q["EXISTS"])) == [["Ana"], ["Luis"]]
    assert rows(api(q["NOT EXISTS"])) == [["Estadistica"]]
    assert rows(api(q["escalar"])) == [["Ana"], ["Pedro"]]   # AVG(20, 17, 22) = 19.67


def test_d4_escalar_con_varias_filas_es_error_de_ejecucion(api):
    ok(api(CURSOS))
    resp = api("SELECT nombre FROM estudiantes WHERE id = (SELECT estudiante_id FROM inscripciones);")
    assert [p["code"] for p in resp["problems"]] == ["EXE003"]


def test_d4_subconsulta_de_dos_columnas_es_error_semantico(api):
    ok(api(CURSOS))
    resp = api("SELECT nombre FROM estudiantes WHERE id IN (SELECT estudiante_id, curso_id FROM inscripciones);")
    assert [p["code"] for p in resp["problems"]] == ["SEM012"]


# ───────────────────────────── E ─────────────────────────────

def test_e_se_detiene_y_conserva_lo_anterior(api):
    resp = api("CREATE TABLE t (a INT PRIMARY KEY);\nINSERT INTO t VALUES (1);\nINSERT INTO t VALUES (1);\nINSERT INTO t VALUES (2);")
    assert [(p["code"], p["line"]) for p in resp["problems"]] == [("EXE001", 3)]
    assert len(resp["statements"]) == 3
    assert rows(api("SELECT a FROM t;")) == [[1]]


def test_e_semantica_antes_de_ejecutar(api):
    resp = api("CREATE TABLE t (a INT);\nINSERT INTO t VALUES ('x');")
    assert [(p["code"], p["line"]) for p in resp["problems"]] == [("SEM008", 2)]
    assert rows(api("SELECT * FROM t;")) == []


def test_e_bases_y_catalogo(api):
    resp = ok(api("CREATE DATABASE u;\nUSE u;\nCREATE TABLE c (id INT PRIMARY KEY);\nINSERT INTO c VALUES (1);"))
    assert resp["database"] == "u"
    cat = api.client.get("/api/catalog?database=u").json()
    assert [(t["name"], t["rows"]) for t in cat["tables"]] == [("c", 1)]
    assert rows(api("SELECT id FROM c;", database="u")) == [[1]]


def test_e_demo_sql_completo(api):
    """workspace/demo.sql se ejecuta entero, sin un solo problema."""
    from pathlib import Path
    text = (Path(main.WORKSPACE_DIR) / "demo.sql").read_text(encoding="utf-8")
    resp = ok(api(text))
    assert all(s["result"] is not None for s in resp["statements"])
    assert len(resp["statements"]) == len(main.build(main.parse(text)))
