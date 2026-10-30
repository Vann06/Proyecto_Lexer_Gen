"""D4 opcional: usar índices en el INNER JOIN (index nested loop).

Si ON tiene una igualdad `tabla_unida.col = <algo de las tablas anteriores>`
y `tabla_unida.col` tiene un índice de una columna, para cada fila de la
izquierda se busca en el índice en vez de recorrer la tabla unida.

Lo que estas pruebas exigen:
1. El resultado es exactamente el mismo que sin índice (comparación con
   datos aleatorios con NULL y repetidos).
2. La tabla unida no se recorre: `Database.read_rows` nunca se llama para
   ella.
3. `Result.plan` dice qué índice usó cada JOIN, para poder demostrarlo.
4. Si ON no es indexable (no hay igualdad, OR, una expresión sobre la
   columna, o la columna no tiene índice) se vuelve al bucle anidado, con
   el mismo resultado.
"""

import random

import pytest

from dbms.storage import Database
from test_executor import db, run  # noqa: F401  (fixture)
from universidad import UNIVERSIDAD


@pytest.fixture
def uni(db):
    run(db, UNIVERSIDAD)
    return db


@pytest.fixture
def reads(monkeypatch):
    """Nombres de las tablas que se leyeron completas con read_rows."""
    seen = []
    original = Database.read_rows

    def spy(self, table):
        seen.append(table.name)
        return original(self, table)

    monkeypatch.setattr(Database, "read_rows", spy)
    return seen


def test_usa_la_pk_implicita_de_la_tabla_unida(uni, reads):
    # cursos.id es PRIMARY KEY: tiene el índice implícito pk_cursos.
    r = run(uni, "SELECT i.nota, c.nombre FROM inscripciones i INNER JOIN cursos c ON c.id = i.curso_id "
                 "ORDER BY i.estudiante_id, i.curso_id")
    assert r.rows == [[91.0, "Compiladores"], [55.5, "Redes"], [80.0, "Redes"],
                      [70.0, "Compiladores"], [None, "Bases de datos"]]
    assert "JOIN c: índice pk_cursos" in r.plan
    assert "cursos" not in reads                       # cursos no se recorrió
    assert "inscripciones" in reads                    # la de la izquierda sí (no hay WHERE)


def test_igualdad_escrita_al_reves_y_sin_calificar(uni, reads):
    r = run(uni, "SELECT nota, nombre FROM inscripciones INNER JOIN cursos ON curso_id = id "
                 "WHERE estudiante_id = 1 ORDER BY nota")
    assert r.rows == [[55.5, "Redes"], [91.0, "Compiladores"]]
    assert "JOIN cursos: índice pk_cursos" in r.plan
    assert "cursos" not in reads


def test_tres_tablas_cada_join_con_su_indice(uni, reads):
    run(uni, "CREATE INDEX idx_insc_est ON inscripciones (estudiante_id)")
    reads.clear()
    r = run(uni, "SELECT e.nombre, c.nombre FROM estudiantes e "
                 "INNER JOIN inscripciones i ON i.estudiante_id = e.id "
                 "INNER JOIN cursos c ON c.id = i.curso_id "
                 "WHERE i.nota >= 70 ORDER BY c.nombre, e.nombre")
    assert r.rows == [["Ana", "Compiladores"], ["Marta", "Compiladores"], ["Luis", "Redes"]]
    assert "JOIN i: índice idx_insc_est" in r.plan and "JOIN c: índice pk_cursos" in r.plan
    assert reads == ["estudiantes"]                    # solo la primera tabla se recorre


def test_condiciones_extra_en_on_se_siguen_evaluando(uni):
    r = run(uni, "SELECT i.estudiante_id, c.nombre FROM inscripciones i INNER JOIN cursos c "
                 "ON c.id = i.curso_id AND c.creditos >= 4 AND i.nota IS NOT NULL ORDER BY i.estudiante_id")
    assert r.rows == [[1, "Compiladores"], [3, "Compiladores"]]
    assert "índice pk_cursos" in r.plan


def test_clave_null_no_coincide_con_nada(uni):
    run(uni, "CREATE TABLE tutor (id INT PRIMARY KEY, alumno INT); "
             "INSERT INTO tutor VALUES (1, 2), (2, NULL), (3, 99)")
    r = run(uni, "SELECT t.id, e.nombre FROM tutor t INNER JOIN estudiantes e ON e.id = t.alumno")
    assert r.rows == [[1, "Luis"]]
    assert "índice pk_estudiantes" in r.plan


@pytest.mark.parametrize("on", [
    "c.id > i.curso_id",                       # no es igualdad
    "c.id = i.curso_id OR c.id = 99",          # OR
    "c.id + 0 = i.curso_id",                   # expresión sobre la columna indexada
    "c.creditos = i.curso_id + 2",             # columna sin índice
])
def test_on_no_indexable_vuelve_al_bucle_anidado(uni, reads, on):
    r = run(uni, f"SELECT i.estudiante_id, i.curso_id, c.id FROM inscripciones i INNER JOIN cursos c ON {on} "
                 "ORDER BY i.estudiante_id, i.curso_id, c.id")
    assert "JOIN c: recorrido completo" in r.plan
    assert "cursos" in reads
    # Mismo resultado que el bucle anidado calculado a mano en Python.
    rows = {"i": run(uni, "SELECT estudiante_id, curso_id FROM inscripciones").rows,
            "c": run(uni, "SELECT id, creditos FROM cursos").rows}
    checks = {
        "c.id > i.curso_id": lambda e, ci, c, cr: c > ci,
        "c.id = i.curso_id OR c.id = 99": lambda e, ci, c, cr: c == ci,
        "c.id + 0 = i.curso_id": lambda e, ci, c, cr: c == ci,
        "c.creditos = i.curso_id + 2": lambda e, ci, c, cr: cr == ci + 2,
    }
    expected = sorted([e, ci, c] for e, ci in rows["i"] for c, cr in rows["c"] if checks[on](e, ci, c, cr))
    assert r.rows == expected


def test_aleatorio_mismo_resultado_con_y_sin_indice(db, reads):
    rnd = random.Random(29)

    def value(lo, hi):
        return "NULL" if rnd.random() < 0.15 else str(rnd.randint(lo, hi))

    run(db, "CREATE TABLE a (id INT PRIMARY KEY, k INT, f FLOAT)")
    run(db, "CREATE TABLE b (id INT PRIMARY KEY, k INT, v INT)")
    run(db, "INSERT INTO a VALUES " + ", ".join(f"({i}, {value(0, 12)}, {value(0, 5)})" for i in range(60)))
    run(db, "INSERT INTO b VALUES " + ", ".join(f"({i}, {value(0, 12)}, {value(0, 9)})" for i in range(80)))
    queries = [
        "SELECT a.id, b.id FROM a INNER JOIN b ON b.k = a.k ORDER BY a.id, b.id",
        "SELECT a.id, b.id FROM a INNER JOIN b ON a.k = b.k AND b.v > 3 ORDER BY a.id, b.id",
        "SELECT a.id, b.id FROM a INNER JOIN b ON b.k = a.k WHERE a.f >= 2 ORDER BY a.id, b.id",
        "SELECT a.id, b.id FROM a INNER JOIN b ON b.k = a.f ORDER BY a.id, b.id",   # FLOAT contra INT
        "SELECT b.k, COUNT(*) FROM a INNER JOIN b ON b.k = a.k GROUP BY b.k ORDER BY b.k",
        "SELECT x.id, y.id FROM b x INNER JOIN b y ON y.k = x.v ORDER BY x.id, y.id",  # auto-join
    ]
    expected = [run(db, q).rows for q in queries]
    assert all(e for e in expected)                    # las consultas devuelven algo
    run(db, "CREATE INDEX ib ON b (k)")
    for q, exp in zip(queries, expected):
        reads.clear()
        r = run(db, q)
        assert r.rows == exp, q
        assert "índice ib" in r.plan, q
        # Solo se recorre la tabla del FROM; la unida se lee por el índice.
        assert reads == [q.split(" FROM ")[1].split()[0]], (q, reads)
