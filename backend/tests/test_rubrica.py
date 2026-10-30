"""Rúbrica del enunciado: un test por requisito, de punta a punta.

Cada test se llama `test_R<nn>_<requisito>` y prueba ese requisito por la
misma vía que el IDE (POST /api/sql/parse y /api/sql/run) sobre una base
nueva con datos "extremos": comillas y comas dentro de cadenas, Unicode,
NULL en cada tipo, fecha bisiesta, negativos, una tabla vacía, un estudiante
sin inscripciones y un curso sin inscritos.

Si algo se rompe, el nombre del test que falla dice qué requisito dejó de
cumplirse:

    python -m pytest tests/test_rubrica.py -v

El último test (R30) encadena todo en un solo caso; cada paso tiene nombre,
así que el mensaje de error dice en qué paso falló.
"""

import re
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

import api.main as main
from dbms import index
from dbms.storage import Database, DataDir

BACKEND = Path(__file__).resolve().parents[1]

BASE = """
CREATE TABLE estudiantes (
    id       INT PRIMARY KEY,
    nombre   VARCHAR(40) NOT NULL,
    carne    VARCHAR(8) UNIQUE,
    edad     INT,
    promedio FLOAT,
    activo   BOOLEAN,
    ingreso  DATE
);
CREATE TABLE cursos (id INT PRIMARY KEY, nombre VARCHAR(30) NOT NULL, creditos INT);
CREATE TABLE inscripciones (estudiante_id INT, curso_id INT, nota FLOAT,
                            PRIMARY KEY (estudiante_id, curso_id),
                            FOREIGN KEY (estudiante_id) REFERENCES estudiantes (id),
                            FOREIGN KEY (curso_id) REFERENCES cursos (id));
CREATE TABLE vacia (n INT);
INSERT INTO estudiantes VALUES
    (1, 'Ana',            'A-001', 20,   88.5,  TRUE,  '2023-01-15'),
    (2, 'Luis',           'A-002', 17,   75.0,  TRUE,  '2024-01-10'),
    (3, 'O''Brien, José', NULL,    NULL, 90.25, FALSE, '2024-02-29'),
    (4, 'Zoë "Z" Ñandú',  'A-004', 22,   NULL,  TRUE,  NULL),
    (5, 'Pedro',          'A-005', 19,   -1.5,  NULL,  '2000-01-01'),
    (6, 'Sofía',          NULL,    16,   100,   TRUE,  '2025-12-31');
INSERT INTO cursos VALUES (1, 'Compiladores', 5), (2, 'Bases de datos', 4), (3, 'Redes', 3), (4, 'Estadística', 2);
INSERT INTO inscripciones VALUES
    (1, 1, 91), (1, 3, 55.5), (2, 3, 80), (3, 1, NULL), (3, 2, 70), (4, 1, 45), (4, 3, 62), (5, 2, 100);
"""
OBRIEN, ZOE = "O'Brien, José", 'Zoë "Z" Ñandú'


class Bd:
    """Una base nueva con BASE cargado, usada a través de la API."""

    def __init__(self, client, data):
        self.client, self.data = client, data
        self.ok(BASE)

    @property
    def path(self) -> Path:
        return self.data / "principal"

    def run(self, sql):
        return self.client.post("/api/sql/run", json={"script": sql}).json()

    def parse(self, sql):
        return self.client.post("/api/sql/parse", json={"script": sql}).json()

    def ok(self, sql):
        resp = self.run(sql)
        assert resp["ok"], f"debía ejecutarse sin errores: {resp['problems']}"
        return resp

    def result(self, sql):
        return self.ok(sql)["statements"][-1]["result"]

    def rows(self, sql):
        return self.result(sql)["rows"]

    def col(self, sql):
        return [r[0] for r in self.rows(sql)]

    def error(self, sql, code, token):
        """`sql` debe fallar con `code`, señalando la línea y columna donde
        empieza `token`."""
        resp = self.run(sql)
        errs = [p for p in resp["problems"] if p["level"] == "err"]
        assert errs, f"debía fallar con {code} y no falló"
        p = errs[-1] if code.startswith(("SEM", "EXE")) else errs[0]
        assert p["code"] == code, f"esperaba {code}, salió {p['code']}: {p['msg']}"
        line = sql.splitlines()[p["line"] - 1]
        assert line[p["col"] - 1:].startswith(token), \
            f"{code} señala la columna {p['col']} ({line[p['col'] - 1:][:15]!r}) y no {token!r}"
        return p


@pytest.fixture
def bd(tmp_path, monkeypatch):
    monkeypatch.setattr(main, "DATA_DIR", tmp_path / "data")
    return Bd(TestClient(main.app), tmp_path / "data")


@pytest.fixture
def lecturas(monkeypatch):
    """Tablas leídas completas con Database.read_rows (para probar índices)."""
    seen = []
    original = Database.read_rows
    monkeypatch.setattr(Database, "read_rows", lambda self, t: seen.append(t.name) or original(self, t))
    return seen


# ═════════════════════════════ Funcionamiento esperado ═════════════════════════════

def test_R01_analisis_lexico_tokens_y_errores(bd):
    """Análisis léxico: tokens con su tipo y posición; errores con ubicación."""
    toks = bd.parse("select Nombre FROM t WHERE x = 'O''Brien' -- comentario\n AND y >= 1.5e2;")["tokens"]
    kinds = [t["kind"] for t in toks]
    assert kinds[:4] == ["SELECT", "IDENTIFIER", "FROM", "IDENTIFIER"]     # sin distinguir mayúsculas
    assert {"kind": "STRING_LIT", "lexeme": "'O''Brien'", "line": 1, "col": 32} in toks
    assert "FLOAT_LIT" in kinds and not any("comentario" in t["lexeme"] for t in toks)
    bd.error("SELECT @ FROM t;", "LEX001", "@")
    bd.error("SELECT * FROM t WHERE a = 'abc", "LEX002", "'abc")


def test_R02_analisis_sintactico_errores_con_ubicacion(bd):
    """Análisis sintáctico: errores en español con línea y columna."""
    bd.error("SELECT nombre FROM WHERE edad >= 18;", "SYN002", "WHERE")
    bd.error("CREATE TABLE t (id INT,);", "SYN001", ")")
    bd.error("UPDATE t SET a 1;", "SYN003", "1")
    bd.error("CREATE TABLA x (id INT);", "SYN004", "TABLA")
    msg = bd.run("SELECT a FROM t t2 t3;")["problems"][0]["msg"]
    assert "fin de archivo" in msg and "fin, de" not in msg


def test_R03_ast_y_arbol_sintactico(bd):
    """El sistema produce el árbol sintáctico y el AST (los dibuja el IDE)."""
    r = bd.parse("SELECT nombre FROM estudiantes WHERE id IN (SELECT estudiante_id FROM inscripciones) "
                 "AND NOT EXISTS (SELECT 1 FROM cursos) AND edad > (SELECT AVG(edad) FROM estudiantes);")
    assert r["problems"] == []
    assert r["parse_tree_dot"].startswith("digraph ParseTree") and "predicate · inSubqueryPred" in r["parse_tree_dot"]
    ast_dot = r["ast_dot"]
    for nodo in ("Select", "InSubquery", "Exists\\nnegated", "ScalarSubquery", "FuncCall\\nname: AVG"):
        assert nodo in ast_dot, f"falta el nodo {nodo} en el AST"
    assert bd.parse("SELECT FROM;")["ast_dot"] == ""       # con errores de sintaxis no hay AST


def test_R04_analisis_semantico_antes_de_ejecutar(bd):
    """La semántica corre antes: una sentencia con error no toca los archivos."""
    antes = (bd.path / "cursos.csv").read_text(encoding="utf-8")
    bd.error("INSERT INTO cursos VALUES (9, 'Física', 'cuatro');", "SEM008", "'cuatro'")
    assert (bd.path / "cursos.csv").read_text(encoding="utf-8") == antes


# ═════════════════════════════ Almacenamiento y tipos ═════════════════════════════

def test_R05_almacenamiento_solo_archivos_y_directorios(bd):
    """Base = directorio; esquema, tipos y restricciones en metadata.txt; un .csv por tabla."""
    files = {p.name for p in bd.path.iterdir()}
    assert {"metadata.txt", "estudiantes.csv", "cursos.csv", "inscripciones.csv", "vacia.csv"} <= files
    meta = (bd.path / "metadata.txt").read_text(encoding="utf-8")
    for linea in ("COLUMN id INT NOT NULL", "COLUMN nombre VARCHAR(40) NOT NULL", "COLUMN promedio FLOAT",
                  "COLUMN activo BOOLEAN", "COLUMN ingreso DATE", "PRIMARY KEY pk_estudiantes (id)",
                  "UNIQUE uq_estudiantes_carne (carne)",
                  "FOREIGN KEY fk_inscripciones_curso_id (curso_id) REFERENCES cursos (id)"):
        assert linea in meta, f"metadata.txt no tiene {linea!r}"
    csv = (bd.path / "estudiantes.csv").read_text(encoding="utf-8").splitlines()
    assert csv[0] == "_rowid,id,nombre,carne,edad,promedio,activo,ingreso"
    assert csv[3] == '3,3,"O\'Brien, José",\\N,\\N,90.25,FALSE,2024-02-29'


def test_R06_sin_ningun_dbms(bd):
    """No se usa SQLite, MySQL, PostgreSQL ni otro DBMS."""
    prohibidos = re.compile(r"^\s*(import|from)\s+(sqlite3|mysql|psycopg2?|pymysql|sqlalchemy|pymongo|duckdb)\b", re.M)
    for py in BACKEND.glob("**/*.py"):
        if "tests" not in py.parts:
            assert not prohibidos.search(py.read_text(encoding="utf-8")), f"{py} importa un DBMS"
    reqs = (BACKEND / "requirements.txt").read_text(encoding="utf-8").lower()
    assert not any(db in reqs for db in ("sqlite", "mysql", "psycopg", "sqlalchemy", "duckdb"))


def test_R07_tipos_de_datos(bd):
    """INT, FLOAT, VARCHAR(n), BOOLEAN, DATE: se guardan, se leen y se validan."""
    assert bd.rows("SELECT id, nombre, promedio, activo, ingreso FROM estudiantes WHERE id = 3;") == \
        [[3, OBRIEN, 90.25, False, "2024-02-29"]]
    assert bd.rows("SELECT promedio FROM estudiantes WHERE id = 6;") == [[100.0]]      # INT -> FLOAT
    assert bd.rows("SELECT nombre FROM estudiantes WHERE id = 4;") == [[ZOE]]          # Unicode y comillas
    bd.error("INSERT INTO estudiantes (id, nombre) VALUES (9, 'Un nombre demasiado largo para cuarenta car');",
             "SEM008", "'Un nombre")                                                   # VARCHAR(40)
    bd.error("INSERT INTO estudiantes (id, nombre, ingreso) VALUES (9, 'X', '2023-02-29');", "SEM008", "'2023")
    bd.error("INSERT INTO estudiantes (id, nombre, activo) VALUES (9, 'X', 1);", "SEM008", "1)")
    bd.error("INSERT INTO estudiantes (id, nombre, edad) VALUES (9, 'X', 1.5);", "SEM008", "1.5")


# ═════════════════════════════ SQL requerido ═════════════════════════════

def test_R08_create_table_y_drop_table(bd):
    bd.ok("CREATE TABLE t (a INT); CREATE TABLE IF NOT EXISTS t (b INT);")
    assert (bd.path / "t.csv").is_file()
    bd.ok("DROP TABLE t; DROP TABLE IF EXISTS t;")
    assert not (bd.path / "t.csv").exists()
    bd.error("SELECT * FROM t;", "SEM001", "t;")


def test_R09_create_index_y_drop_index(bd):
    bd.ok("CREATE INDEX idx_edad ON estudiantes (edad);")
    assert (bd.path / "idx_edad.idx").is_file()
    assert "INDEX idx_edad ON estudiantes (edad)" in (bd.path / "metadata.txt").read_text(encoding="utf-8")
    bd.ok("DROP INDEX idx_edad;")
    assert not (bd.path / "idx_edad.idx").exists()


def test_R10_insert(bd):
    bd.ok("INSERT INTO estudiantes (id, nombre) VALUES (7, 'Nuevo'), (8, 'Otro');")
    assert bd.rows("SELECT carne, edad, activo FROM estudiantes WHERE id = 7;") == [[None, None, None]]
    bd.error("INSERT INTO estudiantes (id, nombre) VALUES (1, 'Repetida');", "EXE001", "INSERT")   # PK
    bd.error("INSERT INTO estudiantes (id, nombre, carne) VALUES (9, 'X', 'A-001');", "EXE001", "INSERT")  # UNIQUE


def test_R11_update(bd):
    r = bd.result("UPDATE estudiantes SET edad = edad + 1, activo = TRUE WHERE edad < 18;")
    assert r["message"] == "2 filas afectadas"
    assert bd.rows("SELECT id, edad, activo FROM estudiantes WHERE id IN (2, 6) ORDER BY id;") == \
        [[2, 18, True], [6, 17, True]]


def test_R12_delete(bd):
    assert bd.result("DELETE FROM inscripciones WHERE nota IS NULL OR nota < 50;")["message"] == "2 filas afectadas"
    assert bd.rows("SELECT COUNT(*) FROM inscripciones;") == [[6]]


def test_R13_select_from_where(bd):
    assert bd.col("SELECT nombre FROM estudiantes WHERE edad >= 18 ORDER BY id;") == ["Ana", ZOE, "Pedro"]
    assert bd.col("SELECT nombre FROM estudiantes WHERE ingreso >= '2024-01-01' ORDER BY id;") == \
        ["Luis", OBRIEN, "Sofía"]
    r = bd.result("SELECT * FROM cursos WHERE creditos = 2;")
    assert r["columns"] == ["id", "nombre", "creditos"] and r["rows"] == [[4, "Estadística", 2]]


def test_R14_and_or_con_null(bd):
    """AND / OR con lógica de tres valores: una comparación con NULL no es verdadera."""
    q = "SELECT id FROM estudiantes WHERE {} ORDER BY id;"
    assert bd.col(q.format("activo = TRUE AND edad >= 18")) == [1, 4]
    assert bd.col(q.format("NOT (activo = TRUE)")) == [3]                       # Pedro (NULL) no entra
    assert bd.col(q.format("edad < 18 OR promedio > 89")) == [2, 3, 6]          # Zoë: FALSE OR NULL
    assert bd.col(q.format("edad IN (16, 22, NULL)")) == [4, 6]
    assert bd.col(q.format("edad NOT IN (16, 22, NULL)")) == []                 # NULL en la lista


def test_R15_order_by(bd):
    assert bd.col("SELECT id FROM estudiantes ORDER BY edad DESC, nombre;") == [4, 1, 5, 2, 6, 3]
    assert bd.col("SELECT id FROM estudiantes ORDER BY promedio;") == [5, 2, 1, 3, 6, 4]   # NULL al final
    assert bd.col("SELECT id, promedio * 2 AS doble FROM estudiantes ORDER BY doble DESC;") == [6, 3, 1, 2, 5, 4]


def test_R16_inner_join(bd):
    r = bd.rows("SELECT e.nombre, c.nombre, i.nota FROM estudiantes e "
                "INNER JOIN inscripciones i ON i.estudiante_id = e.id "
                "INNER JOIN cursos c ON c.id = i.curso_id ORDER BY e.id, c.id;")
    assert r == [["Ana", "Compiladores", 91.0], ["Ana", "Redes", 55.5], ["Luis", "Redes", 80.0],
                 [OBRIEN, "Compiladores", None], [OBRIEN, "Bases de datos", 70.0],
                 [ZOE, "Compiladores", 45.0], [ZOE, "Redes", 62.0], ["Pedro", "Bases de datos", 100.0]]
    bd.error("SELECT nombre FROM estudiantes e INNER JOIN cursos c ON c.id = e.id;", "SEM004", "nombre")


def test_R17_count_sum_avg_min_max(bd):
    assert bd.rows("SELECT COUNT(*), COUNT(nota), SUM(nota), MIN(nota), MAX(nota) FROM inscripciones;") == \
        [[8, 7, 503.5, 45.0, 100.0]]
    [[avg]] = bd.rows("SELECT AVG(nota) FROM inscripciones;")
    assert avg == pytest.approx(503.5 / 7)
    assert bd.rows("SELECT COUNT(DISTINCT curso_id) FROM inscripciones;") == [[3]]
    assert bd.rows("SELECT COUNT(*), SUM(n), AVG(n), MIN(n) FROM vacia;") == [[0, None, None, None]]


def test_R18_group_by_y_having(bd):
    r = bd.rows("SELECT c.nombre, COUNT(*) AS n, COUNT(i.nota), AVG(i.nota) AS prom, MIN(i.nota), MAX(i.nota) "
                "FROM cursos c INNER JOIN inscripciones i ON i.curso_id = c.id "
                "GROUP BY c.nombre HAVING COUNT(*) >= 3 ORDER BY prom DESC;")
    assert r == [["Compiladores", 3, 2, 68.0, 45.0, 91.0], ["Redes", 3, 3, pytest.approx(197.5 / 3), 55.5, 80.0]]
    assert bd.rows("SELECT e.nombre, SUM(c.creditos) AS cr FROM estudiantes e "
                   "INNER JOIN inscripciones i ON i.estudiante_id = e.id INNER JOIN cursos c ON c.id = i.curso_id "
                   "GROUP BY e.nombre HAVING SUM(c.creditos) >= 8 ORDER BY cr DESC, e.nombre;") == \
        [[OBRIEN, 9], ["Ana", 8], [ZOE, 8]]
    assert bd.rows("SELECT activo, COUNT(*) FROM estudiantes GROUP BY activo ORDER BY activo;") == \
        [[False, 1], [True, 4], [None, 1]]                                         # NULL es un grupo
    bd.error("SELECT curso_id, nota FROM inscripciones GROUP BY curso_id;", "SEM013", "nota")


def test_R19_in_y_exists(bd):
    assert bd.col("SELECT nombre FROM cursos WHERE id IN (2, 4) ORDER BY id;") == ["Bases de datos", "Estadística"]
    assert bd.col("SELECT nombre FROM cursos c WHERE EXISTS "
                  "(SELECT 1 FROM inscripciones i WHERE i.curso_id = c.id) ORDER BY c.id;") == \
        ["Compiladores", "Bases de datos", "Redes"]


# ═════════════════════════════ Índices ═════════════════════════════

def test_R20_where_usa_el_indice_y_no_recorre_la_tabla(bd, lecturas):
    bd.ok("CREATE INDEX idx_edad ON estudiantes (edad);")
    for where, esperado in [("edad >= 18", [1, 4, 5]), ("edad = 17", [2]), ("edad > 16 AND edad < 20", [2, 5]),
                            ("18 <= edad", [1, 4, 5])]:
        lecturas.clear()
        r = bd.result(f"SELECT id FROM estudiantes WHERE {where} ORDER BY id;")
        assert [row[0] for row in r["rows"]] == esperado, where
        assert r["plan"] == "índice idx_edad", f"{where}: plan {r['plan']}"
        assert "estudiantes" not in lecturas, f"{where}: recorrió la tabla aunque había índice"
    assert bd.result("SELECT id FROM estudiantes WHERE edad = 20 OR edad = 17;")["plan"] == "recorrido completo"


def test_R21_indices_actualizados_tras_insert_update_delete(bd):
    bd.ok("CREATE INDEX idx_edad ON estudiantes (edad); CREATE INDEX idx_nota ON inscripciones (nota);")
    bd.ok("INSERT INTO estudiantes (id, nombre, edad) VALUES (7, 'Nuevo', 18);"
          "UPDATE estudiantes SET edad = edad + 1 WHERE edad < 18;"
          "DELETE FROM estudiantes WHERE id = 1;"
          "UPDATE inscripciones SET nota = 50 WHERE nota IS NULL;"
          "DELETE FROM inscripciones WHERE nota < 60;")
    db = DataDir(bd.data).open()
    catalog = db.load_catalog()
    for nombre in ("idx_edad", "idx_nota", "pk_estudiantes", "pk_inscripciones"):
        idx = catalog.index(nombre)
        actual = (db.path / f"{nombre}.idx").read_text(encoding="utf-8")
        assert actual == index.build_text(idx, db.read_rows(catalog.table(idx.table))), f"{nombre}.idx desactualizado"
    r = bd.result("SELECT nombre FROM estudiantes WHERE edad = 18 ORDER BY id;")
    assert (r["rows"], r["plan"]) == ([["Luis"], ["Nuevo"]], "índice idx_edad")


# ═════════════════════════════ Subconsultas ═════════════════════════════

def test_R22_subconsulta_ejemplo_del_enunciado(bd):
    assert bd.col("SELECT nombre\nFROM estudiantes\nWHERE id IN (SELECT estudiante_id\n"
                  "             FROM inscripciones\n             WHERE curso_id = 3);") == ["Ana", "Luis", ZOE]


def test_R23_subconsulta_in_y_not_in(bd):
    assert bd.col("SELECT nombre FROM estudiantes WHERE id NOT IN (SELECT estudiante_id FROM inscripciones);") == \
        ["Sofía"]
    # La subconsulta devuelve un NULL: `x NOT IN (..., NULL)` nunca es verdadero.
    assert bd.col("SELECT nombre FROM estudiantes WHERE promedio NOT IN (SELECT nota FROM inscripciones);") == []
    assert bd.col("SELECT nombre FROM estudiantes WHERE id IN (SELECT estudiante_id FROM inscripciones "
                  "WHERE curso_id IN (SELECT id FROM cursos WHERE creditos >= 4)) ORDER BY id;") == \
        ["Ana", OBRIEN, ZOE, "Pedro"]


def test_R24_subconsulta_exists_y_not_exists(bd):
    assert bd.col("SELECT e.nombre FROM estudiantes e WHERE EXISTS "
                  "(SELECT 1 FROM inscripciones i WHERE i.estudiante_id = e.id AND i.nota >= 90) ORDER BY e.id;") == \
        ["Ana", "Pedro"]
    assert bd.col("SELECT c.nombre FROM cursos c WHERE NOT EXISTS "
                  "(SELECT 1 FROM inscripciones i WHERE i.curso_id = c.id);") == ["Estadística"]


def test_R25_subconsulta_escalar_en_comparaciones(bd):
    # AVG(edad) ignora el NULL: 94 / 5 = 18.8
    assert bd.col("SELECT id FROM estudiantes WHERE edad > (SELECT AVG(edad) FROM estudiantes) ORDER BY id;") == \
        [1, 4, 5]
    assert bd.col("SELECT id FROM estudiantes WHERE edad = (SELECT MAX(edad) FROM estudiantes);") == [4]
    assert bd.col("SELECT id FROM cursos WHERE creditos < (SELECT AVG(creditos) FROM cursos) ORDER BY id;") == [3, 4]
    assert bd.rows("SELECT e.id, (SELECT COUNT(*) FROM inscripciones i WHERE i.estudiante_id = e.id) "
                   "FROM estudiantes e ORDER BY e.id;") == [[1, 2], [2, 1], [3, 2], [4, 2], [5, 1], [6, 0]]


# ═════════════════════════════ Análisis semántico ═════════════════════════════

SEMANTICA = [
    ("tabla inexistente", "SELECT * FROM profesores;", "SEM001", "profesores"),
    ("columna inexistente", "SELECT apellido FROM estudiantes;", "SEM003", "apellido"),
    ("tipos incompatibles", "SELECT nombre FROM estudiantes WHERE edad >= 'dieciocho';", "SEM006", "edad"),
    ("INSERT con cantidad de valores incorrecta", "INSERT INTO cursos VALUES (9, 'Física');", "SEM007", "9"),
    ("INSERT con valor de otro tipo", "INSERT INTO cursos VALUES (9, 'Física', 'cuatro');", "SEM008", "'cuatro'"),
    ("INSERT con NULL en NOT NULL", "INSERT INTO cursos VALUES (9, NULL, 4);", "SEM009", "NULL"),
    ("referencia ambigua", "SELECT id FROM estudiantes e INNER JOIN cursos c ON c.id = e.id;", "SEM004", "id FROM"),
    ("referencia inválida", "SELECT x.nombre FROM estudiantes e;", "SEM005", "x.nombre"),
    ("índice inexistente", "DROP INDEX idx_nada;", "SEM010", "DROP"),
    ("índice duplicado", "CREATE INDEX i1 ON cursos (creditos);\nCREATE INDEX i1 ON cursos (nombre);", "SEM011", "CREATE"),
    ("escalar con más de una columna", "SELECT id FROM estudiantes WHERE edad = (SELECT edad, id FROM estudiantes);",
     "SEM012", "(SELECT"),
    ("escalar con más de un registro", "SELECT id FROM estudiantes WHERE edad = (SELECT edad FROM estudiantes);",
     "EXE003", "(SELECT"),
]


def _ascii(texto: str) -> str:
    import unicodedata
    return unicodedata.normalize("NFKD", texto).encode("ascii", "ignore").decode().replace(" ", "_")


@pytest.mark.parametrize("caso, sql, code, token", SEMANTICA, ids=[_ascii(c[0]) for c in SEMANTICA])
def test_R26_errores_semanticos_con_ubicacion(bd, caso, sql, code, token):
    """Cada error de la lista del enunciado, con su código y su ubicación exacta."""
    bd.error(sql, code, token)


# ═════════════════════════════ Interfaz ═════════════════════════════

def test_R27_interfaz_grafica(bd):
    """El IDE ofrece: escribir SQL, tokens, árbol, AST, errores, ejecutar y resultados."""
    jsx = (BACKEND.parent / "frontend" / "IDE-lite" / "app.jsx").read_text(encoding="utf-8")
    for etiqueta in ('label:"TOKENS"', 'label:"ÁRBOL SINTÁCTICO"', 'label:"AST"', 'label:"PROBLEMAS"',
                     'label:"RESULTADOS"', 'label:"CATÁLOGO"', "ANALIZAR", "EJECUTAR", "<textarea"):
        assert etiqueta in jsx, f"el IDE no tiene {etiqueta}"
    for endpoint in ("/api/sql/parse", "/api/sql/run", "/api/catalog"):
        assert endpoint in jsx, f"el IDE no llama a {endpoint}"
    cat = bd.client.get("/api/catalog").json()
    assert {t["name"]: t["rows"] for t in cat["tables"]} == {"estudiantes": 6, "cursos": 4, "inscripciones": 8, "vacia": 0}


def test_R28_ejemplo_del_enunciado(bd):
    """SELECT nombre FROM estudiantes WHERE edad >= 18: solo los que cumplen."""
    r = bd.result("SELECT nombre\nFROM estudiantes\nWHERE edad >= 18;")
    assert r["columns"] == ["nombre"] and sorted(r["rows"]) == sorted([["Ana"], [ZOE], ["Pedro"]])


def test_R29_persistencia_entre_ejecuciones(bd):
    """Los datos viven en los archivos: otra ejecución (otra petición) los ve."""
    bd.ok("INSERT INTO cursos VALUES (5, 'Física', 4);")
    fresh = TestClient(main.app)
    r = fresh.post("/api/sql/run", json={"script": "SELECT nombre FROM cursos WHERE id = 5;"}).json()
    assert r["statements"][-1]["result"]["rows"] == [["Física"]]


# ═════════════════════════════ Caso extremo de punta a punta ═════════════════════════════

PASOS = [
    ("crear índices", "CREATE INDEX idx_edad ON estudiantes (edad); CREATE INDEX idx_insc ON inscripciones (estudiante_id);",
     None),
    ("boleta con JOIN usando índices",
     "SELECT e.nombre, COUNT(*) FROM estudiantes e INNER JOIN inscripciones i ON i.estudiante_id = e.id "
     "GROUP BY e.nombre HAVING COUNT(*) = 2 ORDER BY e.nombre;", [["Ana", 2], [OBRIEN, 2], [ZOE, 2]]),
    ("curva +10 con subconsulta en el WHERE del UPDATE",
     "UPDATE inscripciones SET nota = nota + 10 WHERE nota < (SELECT AVG(nota) FROM inscripciones);", None),
    ("notas tras la curva (el AVG se calculó antes del cambio)",
     "SELECT estudiante_id, curso_id, nota FROM inscripciones WHERE nota IS NOT NULL ORDER BY nota, estudiante_id;",
     [[4, 1, 55.0], [1, 3, 65.5], [4, 3, 72.0], [2, 3, 80.0], [3, 2, 80.0], [1, 1, 91.0], [5, 2, 100.0]]),
    ("baja de quien no tiene inscripciones (NOT EXISTS en DELETE)",
     "DELETE FROM estudiantes WHERE NOT EXISTS (SELECT 1 FROM inscripciones i WHERE i.estudiante_id = estudiantes.id);",
     None),
    ("alta con id calculado por subconsulta escalar",
     "INSERT INTO estudiantes (id, nombre, edad) VALUES ((SELECT MAX(id) FROM estudiantes) + 10, 'Último', 18);", None),
    ("el índice ve los cambios", "SELECT id, nombre FROM estudiantes WHERE edad >= 18 ORDER BY id;",
     [[1, "Ana"], [4, ZOE], [5, "Pedro"], [15, "Último"]]),
    ("promedio por curso con COUNT(DISTINCT) y NULL",
     "SELECT c.nombre, COUNT(i.nota), COUNT(DISTINCT i.estudiante_id), MAX(i.nota) FROM cursos c "
     "INNER JOIN inscripciones i ON i.curso_id = c.id GROUP BY c.nombre ORDER BY c.nombre;",
     [["Bases de datos", 2, 2, 100.0], ["Compiladores", 2, 3, 91.0], ["Redes", 3, 3, 80.0]]),
    ("borrar el índice y repetir: mismo resultado sin él",
     "DROP INDEX idx_edad; SELECT id, nombre FROM estudiantes WHERE edad >= 18 ORDER BY id;",
     [[1, "Ana"], [4, ZOE], [5, "Pedro"], [15, "Último"]]),
]


def test_R30_caso_extremo_de_punta_a_punta(bd):
    """Todo encadenado sobre la misma base; el mensaje dice qué paso falló."""
    for numero, (nombre, sql, esperado) in enumerate(PASOS, start=1):
        resp = bd.run(sql)
        assert resp["ok"], f"Paso {numero} ({nombre}) dio error: {resp['problems']}"
        if esperado is not None:
            obtenido = resp["statements"][-1]["result"]["rows"]
            assert obtenido == esperado, f"Paso {numero} ({nombre}): esperaba {esperado}, salió {obtenido}"
