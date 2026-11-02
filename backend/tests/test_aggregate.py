"""D3: agregados, GROUP BY y HAVING."""

import pytest

from test_executor import db, run  # noqa: F401  (fixture)
from universidad import UNIVERSIDAD


@pytest.fixture
def uni(db):
    run(db, UNIVERSIDAD)
    return db


def one(db, sql):
    [row] = run(db, sql).rows
    return row


def test_funciones_ignoran_null(uni):
    assert one(uni, "SELECT COUNT(*), COUNT(edad), COUNT(promedio) FROM estudiantes") == [4, 3, 3]
    assert one(uni, "SELECT MIN(edad), MAX(edad), SUM(edad) FROM estudiantes") == [17, 22, 59]
    assert one(uni, "SELECT MIN(nombre), MAX(nombre) FROM estudiantes") == ["Ana", "Sofia"]
    assert one(uni, "SELECT COUNT(nota), AVG(nota) FROM inscripciones") == [4, (91 + 55.5 + 80 + 70) / 4]


def test_tipos_de_resultado(uni):
    total, promedio = one(uni, "SELECT SUM(edad), AVG(edad) FROM estudiantes")
    assert type(total) is int and type(promedio) is float
    assert promedio == pytest.approx(59 / 3)
    assert type(one(uni, "SELECT SUM(promedio) FROM estudiantes")[0]) is float


def test_distinct(uni):
    assert one(uni, "SELECT COUNT(DISTINCT curso_id), COUNT(curso_id) FROM inscripciones") == [3, 5]
    assert one(uni, "SELECT SUM(DISTINCT estudiante_id) FROM inscripciones") == [1 + 2 + 3]


def test_tabla_vacia(uni):
    run(uni, "CREATE TABLE vacia (n INT)")
    # Sin GROUP BY: un grupo aunque no haya filas.
    assert run(uni, "SELECT COUNT(*), COUNT(n), SUM(n), AVG(n), MIN(n) FROM vacia").rows == [[0, 0, None, None, None]]
    # Con GROUP BY: ningún grupo.
    assert run(uni, "SELECT n, COUNT(*) FROM vacia GROUP BY n").rows == []
    # WHERE que no deja filas: igual que la tabla vacía.
    assert run(uni, "SELECT COUNT(*) FROM estudiantes WHERE edad > 100").rows == [[0]]


def test_group_by_una_columna(uni):
    r = run(uni, "SELECT curso_id, COUNT(*) AS n, AVG(nota) AS prom FROM inscripciones "
                 "GROUP BY curso_id ORDER BY curso_id")
    assert r.columns == ["curso_id", "n", "prom"]
    assert r.rows == [[1, 2, 80.5], [2, 1, None], [3, 2, 67.75]]


def test_null_forma_su_propio_grupo(uni):
    r = run(uni, "SELECT edad, COUNT(*) FROM estudiantes GROUP BY edad ORDER BY edad")
    assert r.rows == [[17, 1], [20, 1], [22, 1], [None, 1]]
    r = run(uni, "SELECT nota, COUNT(*) FROM inscripciones GROUP BY nota HAVING nota IS NULL")
    assert r.rows == [[None, 1]]


def test_group_by_dos_columnas_y_expresion(uni):
    run(uni, "CREATE TABLE ventas (zona VARCHAR(5), mes INT, monto FLOAT);"
             "INSERT INTO ventas VALUES ('N', 1, 10), ('N', 1, 5), ('N', 2, 7), ('S', 1, 3), ('S', 1, NULL)")
    r = run(uni, "SELECT zona, mes, SUM(monto), COUNT(monto) FROM ventas GROUP BY zona, mes ORDER BY zona, mes")
    assert r.rows == [["N", 1, 15.0, 2], ["N", 2, 7.0, 1], ["S", 1, 3.0, 1]]
    r = run(uni, "SELECT mes * 10, COUNT(*) FROM ventas GROUP BY mes * 10 ORDER BY mes * 10")
    assert r.rows == [[10, 4], [20, 1]]


def test_having_y_order_by_por_agregado(uni):
    r = run(uni, "SELECT estudiante_id, COUNT(*) FROM inscripciones GROUP BY estudiante_id "
                 "HAVING COUNT(*) >= 2 ORDER BY estudiante_id")
    assert r.rows == [[1, 2], [3, 2]]
    r = run(uni, "SELECT estudiante_id FROM inscripciones GROUP BY estudiante_id ORDER BY MAX(nota) DESC")
    assert r.rows == [[1], [2], [3]]
    r = run(uni, "SELECT curso_id, SUM(nota) AS total FROM inscripciones GROUP BY curso_id ORDER BY total DESC")
    assert r.rows == [[1, 161.0], [3, 135.5], [2, None]]
    # HAVING sin GROUP BY: un solo grupo, que pasa o no.
    assert run(uni, "SELECT COUNT(*) FROM estudiantes HAVING COUNT(*) > 10").rows == []
    assert run(uni, "SELECT COUNT(*) FROM estudiantes HAVING COUNT(*) > 1").rows == [[4]]


def test_expresiones_con_agregados(uni):
    assert one(uni, "SELECT MAX(edad) - MIN(edad), COUNT(*) * 2 FROM estudiantes") == [5, 8]


def test_where_filtra_antes_de_agrupar(uni):
    r = run(uni, "SELECT curso_id, COUNT(*) FROM inscripciones WHERE nota >= 70 GROUP BY curso_id ORDER BY curso_id")
    assert r.rows == [[1, 2], [3, 1]]


def test_join_con_group_by(uni):
    """El criterio de "hecho" de D3 en PLAN.md: conteos y promedios por curso."""
    r = run(uni, "SELECT c.nombre, COUNT(*) AS inscritos, AVG(i.nota) AS promedio "
                 "FROM cursos c INNER JOIN inscripciones i ON i.curso_id = c.id "
                 "GROUP BY c.nombre HAVING COUNT(*) >= 1 ORDER BY inscritos DESC, c.nombre")
    assert r.rows == [["Compiladores", 2, 80.5], ["Redes", 2, 67.75], ["Bases de datos", 1, None]]
