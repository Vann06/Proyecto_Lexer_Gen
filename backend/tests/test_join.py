"""D3: INNER JOIN."""

import pytest

from test_executor import db, run  # noqa: F401  (fixture)
from universidad import UNIVERSIDAD


@pytest.fixture
def uni(db):
    run(db, UNIVERSIDAD)
    return db


def test_dos_tablas(uni):
    r = run(uni, "SELECT e.nombre, i.nota FROM estudiantes e INNER JOIN inscripciones i "
                 "ON i.estudiante_id = e.id ORDER BY e.id, i.curso_id")
    assert r.columns == ["nombre", "nota"]
    assert r.rows == [["Ana", 91.0], ["Ana", 55.5], ["Luis", 80.0], ["Marta", 70.0], ["Marta", None]]


def test_tres_tablas_con_where(uni):
    r = run(uni, "SELECT e.nombre, c.nombre FROM estudiantes e "
                 "INNER JOIN inscripciones i ON i.estudiante_id = e.id "
                 "INNER JOIN cursos c ON c.id = i.curso_id "
                 "WHERE i.nota >= 70 ORDER BY c.nombre, e.nombre")
    assert r.rows == [["Ana", "Compiladores"], ["Marta", "Compiladores"], ["Luis", "Redes"]]


def test_columnas_sin_calificar_y_join_sin_alias(uni):
    r = run(uni, "SELECT nombre, nota FROM estudiantes INNER JOIN inscripciones "
                 "ON estudiante_id = id WHERE curso_id = 3 ORDER BY nota")
    assert r.rows == [["Ana", 55.5], ["Luis", 80.0]]


def test_sin_coincidencias(uni):
    assert run(uni, "SELECT e.nombre FROM estudiantes e INNER JOIN inscripciones i "
                    "ON i.estudiante_id = e.id AND e.id > 100").rows == []


def test_estrella_de_una_tabla_y_de_todas(uni):
    r = run(uni, "SELECT c.* FROM cursos c INNER JOIN inscripciones i ON i.curso_id = c.id WHERE i.nota = 91")
    assert (r.columns, r.rows) == (["id", "nombre", "creditos"], [[1, "Compiladores", 5]])
    r = run(uni, "SELECT * FROM cursos c INNER JOIN inscripciones i ON i.curso_id = c.id WHERE i.nota = 91")
    assert r.columns == ["id", "nombre", "creditos", "estudiante_id", "curso_id", "nota"]


def test_auto_join_con_alias(uni):
    # Pares (a, b) con a más joven que b; Marta (edad NULL) no entra en ninguno.
    r = run(uni, "SELECT a.nombre, b.nombre FROM estudiantes a INNER JOIN estudiantes b "
                 "ON a.edad < b.edad ORDER BY a.edad, b.edad")
    assert r.rows == [["Luis", "Ana"], ["Luis", "Sofia"], ["Ana", "Sofia"]]


def test_join_con_indice_en_la_primera_tabla_no_cambia_el_resultado(uni):
    query = ("SELECT e.nombre, i.curso_id FROM estudiantes e INNER JOIN inscripciones i "
             "ON i.estudiante_id = e.id WHERE e.edad >= 18 ORDER BY e.nombre, i.curso_id")
    before = run(uni, query).rows
    run(uni, "CREATE INDEX idx_edad ON estudiantes (edad)")
    assert run(uni, query).rows == before == [["Ana", 1], ["Ana", 3]]
