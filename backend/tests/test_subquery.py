"""D4: subconsultas (IN, EXISTS, escalares) y su caché."""

import pytest

from dbms import subquery
from dbms.builder import build
from dbms.frontend import parse
from dbms.result import ExecutionError
from test_executor import db, run  # noqa: F401  (fixture)
from universidad import UNIVERSIDAD


@pytest.fixture
def uni(db):
    run(db, UNIVERSIDAD)
    return db


def names(db, sql):
    return [row[0] for row in run(db, sql).rows]


def test_in_y_not_in(uni):
    assert names(uni, "SELECT nombre FROM estudiantes WHERE id IN "
                      "(SELECT estudiante_id FROM inscripciones WHERE curso_id = 3) ORDER BY id") == ["Ana", "Luis"]
    assert names(uni, "SELECT nombre FROM estudiantes WHERE id NOT IN "
                      "(SELECT estudiante_id FROM inscripciones)") == ["Sofia"]


def test_not_in_con_null_en_la_subconsulta_no_da_nada(uni):
    # La subconsulta devuelve (NULL, 22): `x NOT IN (NULL, ...)` nunca es TRUE.
    assert names(uni, "SELECT nombre FROM estudiantes WHERE edad NOT IN "
                      "(SELECT edad FROM estudiantes WHERE id >= 3)") == []
    assert names(uni, "SELECT nombre FROM estudiantes WHERE edad IN "
                      "(SELECT edad FROM estudiantes WHERE id >= 3)") == ["Sofia"]


def test_exists_y_not_exists_correlacionados(uni):
    assert names(uni, "SELECT e.nombre FROM estudiantes e WHERE EXISTS "
                      "(SELECT * FROM inscripciones i WHERE i.estudiante_id = e.id AND i.nota >= 80) "
                      "ORDER BY e.id") == ["Ana", "Luis"]
    assert names(uni, "SELECT c.nombre FROM cursos c WHERE NOT EXISTS "
                      "(SELECT 1 FROM inscripciones i WHERE i.curso_id = c.id)") == ["Estadistica"]


def test_escalar_en_where(uni):
    # AVG(edad) = 59/3 ≈ 19.67
    assert names(uni, "SELECT nombre FROM estudiantes WHERE edad > "
                      "(SELECT AVG(edad) FROM estudiantes) ORDER BY id") == ["Ana", "Sofia"]


def test_escalar_correlacionada_en_select_y_grupo_vacio(uni):
    r = run(uni, "SELECT e.nombre, (SELECT COUNT(*) FROM inscripciones i WHERE i.estudiante_id = e.id) AS n, "
                 "(SELECT MAX(nota) FROM inscripciones i WHERE i.estudiante_id = e.id) AS mejor "
                 "FROM estudiantes e ORDER BY e.id")
    assert r.rows == [["Ana", 2, 91.0], ["Luis", 1, 80.0], ["Marta", 2, 70.0], ["Sofia", 0, None]]


def test_escalar_sin_filas_es_null_y_con_varias_es_error(uni):
    assert run(uni, "SELECT (SELECT nota FROM inscripciones WHERE estudiante_id = 4)").rows == [[None]]
    with pytest.raises(ExecutionError) as e:
        run(uni, "SELECT nombre FROM estudiantes WHERE id = (SELECT estudiante_id FROM inscripciones)")
    assert e.value.code == "EXE003"


def test_anidadas(uni):
    # Estudiantes inscritos en algún curso de 4 o más créditos (Compiladores, Bases de datos).
    assert names(uni, "SELECT nombre FROM estudiantes WHERE id IN (SELECT estudiante_id FROM inscripciones "
                      "WHERE curso_id IN (SELECT id FROM cursos WHERE creditos >= 4)) ORDER BY id") == ["Ana", "Marta"]
    # La más interna usa una columna de la más externa (c.creditos).
    assert names(uni, "SELECT c.nombre FROM cursos c WHERE EXISTS (SELECT 1 FROM inscripciones i "
                      "WHERE i.curso_id = c.id AND EXISTS (SELECT 1 FROM estudiantes e "
                      "WHERE e.id = i.estudiante_id AND e.edad < c.creditos * 5))") == ["Compiladores"]


def test_en_update_delete_e_insert(uni):
    run(uni, "UPDATE estudiantes SET promedio = (SELECT AVG(nota) FROM inscripciones i "
             "WHERE i.estudiante_id = estudiantes.id)")
    assert run(uni, "SELECT promedio FROM estudiantes ORDER BY id").rows == [[73.25], [80.0], [70.0], [None]]
    assert run(uni, "DELETE FROM cursos WHERE id NOT IN (SELECT curso_id FROM inscripciones)").message \
        == "1 filas afectadas"
    run(uni, "INSERT INTO cursos VALUES ((SELECT MAX(id) FROM cursos) + 1, 'Nuevo', 1)")
    assert names(uni, "SELECT nombre FROM cursos ORDER BY id") == ["Compiladores", "Bases de datos", "Redes", "Nuevo"]


def test_update_ve_el_estado_anterior(uni):
    # Todas las filas comparan contra el promedio de ANTES del UPDATE.
    run(uni, "UPDATE estudiantes SET edad = edad + 10 WHERE edad < (SELECT AVG(edad) FROM estudiantes)")
    assert run(uni, "SELECT edad FROM estudiantes ORDER BY id").rows == [[20], [27], [None], [22]]


def test_cache_no_correlacionada_una_vez_correlacionada_por_fila(uni, monkeypatch):
    calls = []
    original = subquery.select
    monkeypatch.setattr(subquery, "select", lambda sel, db, env: calls.append(1) or original(sel, db, env))
    run(uni, "SELECT nombre FROM estudiantes WHERE id IN (SELECT estudiante_id FROM inscripciones)")
    assert len(calls) == 1
    calls.clear()
    run(uni, "SELECT nombre FROM estudiantes e WHERE EXISTS "
             "(SELECT 1 FROM inscripciones i WHERE i.estudiante_id = e.id)")
    assert len(calls) == 4          # una por estudiante


@pytest.mark.parametrize("sql, expected", [
    ("SELECT 1 FROM estudiantes WHERE id IN (SELECT estudiante_id FROM inscripciones)", False),
    ("SELECT 1 FROM estudiantes e WHERE EXISTS (SELECT 1 FROM inscripciones i WHERE i.estudiante_id = e.id)", True),
    # Sin calificar: `id` no está en inscripciones, es de la consulta externa.
    ("SELECT 1 FROM estudiantes WHERE EXISTS (SELECT 1 FROM inscripciones WHERE estudiante_id = id)", True),
    # Una anidada que solo usa sus propias tablas no la vuelve correlacionada.
    ("SELECT 1 FROM estudiantes WHERE id IN (SELECT estudiante_id FROM inscripciones i "
     "WHERE EXISTS (SELECT 1 FROM cursos c WHERE c.id = i.curso_id))", False),
    ("SELECT 1 FROM cursos c WHERE EXISTS (SELECT 1 FROM inscripciones i WHERE EXISTS "
     "(SELECT 1 FROM estudiantes e WHERE e.edad < c.creditos))", True),
])
def test_deteccion_de_correlacion(uni, sql, expected):
    [stmt] = build(parse(sql))
    where = stmt.body.where
    assert subquery.is_correlated(where.select, uni.load_catalog()) is expected
