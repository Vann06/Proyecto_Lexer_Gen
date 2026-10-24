import random

import pytest

from dbms.result import ExecutionError
from dbms.storage import Database
from test_executor import db, run, snapshot


@pytest.mark.parametrize("predicate,expected", [
    ("n = 2", [[3], [4]]), ("n < 2", [[1], [2]]), ("n <= 2", [[1], [2], [3], [4]]),
    ("n > 2", [[5]]), ("n >= 2", [[3], [4], [5]]), ("2 < n", [[5]]),
    ("n >= -2 AND n < 2", [[1], [2]]), ("n = NULL", []), ("n > 100", []),
])
def test_indexed_predicates_never_full_scan(db, monkeypatch, predicate, expected):
    run(db, "CREATE TABLE t (id INT, n INT); "
            "INSERT INTO t VALUES (1, -2), (2, 1), (3, 2), (4, 2), (5, 10), (6, NULL); "
            "CREATE INDEX idx_n ON t (n)")
    def forbidden(*args):
        pytest.fail("Una consulta indexada no debe leer toda la tabla")
    monkeypatch.setattr(Database, "read_rows", forbidden)
    result = run(Database(db.path), f"SELECT id FROM t WHERE {predicate} ORDER BY id")
    assert result.plan == "índice idx_n"
    assert result.rows == expected


def test_residual_predicate_and_or_fallback(db):
    run(db, "CREATE TABLE t (id INT, n INT); INSERT INTO t VALUES (1, 2), (2, 2), (3, NULL); "
            "CREATE INDEX idx_n ON t (n)")
    assert run(db, "SELECT id FROM t x WHERE x.n = 2 AND id > 1").rows == [[2]]
    result = run(db, "SELECT id FROM t WHERE n = 2 OR n IS NULL ORDER BY id")
    assert result.rows == [[1], [2], [3]]
    assert result.plan == "recorrido completo"


def test_maintenance_and_drop(db):
    run(db, "CREATE TABLE t (id INT PRIMARY KEY, n INT); CREATE INDEX idx_n ON t (n); "
            "INSERT INTO t VALUES (1, 8), (2, 8), (3, NULL)")
    assert (db.path / "idx_n.idx").read_text() == "8|1,2\nnull|3\n"
    result = run(db, "UPDATE t SET n = n + 1 WHERE n = 8 AND id = 2")
    assert result.plan == "índice idx_n, pk_t"
    assert (db.path / "idx_n.idx").read_text() == "8|1\n9|2\nnull|3\n"
    run(db, "DELETE FROM t WHERE n = 8; INSERT INTO t VALUES (4, 9)")
    assert (db.path / "idx_n.idx").read_text() == "9|2,4\nnull|3\n"
    assert run(Database(db.path), "SELECT id FROM t WHERE n = 9 ORDER BY id").rows == [[2], [4]]
    run(db, "DROP INDEX idx_n; DROP INDEX IF EXISTS idx_n")
    assert not (db.path / "idx_n.idx").exists()
    assert run(db, "SELECT id FROM t WHERE n = 9").plan == "recorrido completo"


def test_unique_index_validation_before_write(db):
    run(db, "CREATE TABLE t (id INT, n INT); INSERT INTO t VALUES (1, 2), (2, 2)")
    before = snapshot(db)
    with pytest.raises(ExecutionError) as error:
        run(db, "CREATE UNIQUE INDEX uq ON t (n)")
    assert error.value.code == "EXE001"
    assert snapshot(db) == before
    run(db, "CREATE UNIQUE INDEX uq ON t (id)")
    before = snapshot(db)
    for sql in ["INSERT INTO t VALUES (1, 4)", "UPDATE t SET id = 1"]:
        with pytest.raises(ExecutionError):
            run(db, sql)
        assert snapshot(db) == before
    run(db, "INSERT INTO t VALUES (NULL, 3), (NULL, 4)")
    run(db, "CREATE INDEX IF NOT EXISTS uq ON t (n)")
    assert db.load_catalog().index("uq").columns == ["id"]


def test_text_date_float_boolean_and_multiline_offsets(db, monkeypatch):
    run(db, """CREATE TABLE t (id INT, s VARCHAR(50), d DATE, f FLOAT, b BOOLEAN);
        INSERT INTO t VALUES (1, 'á|"x
segunda línea', '2026-01-01', -2.5, TRUE),
        (2, '\\N', '2026-10-01', 2, FALSE), (3, '', NULL, NULL, NULL);
        CREATE INDEX idx_s ON t (s); CREATE INDEX idx_d ON t (d);
        CREATE INDEX idx_f ON t (f); CREATE INDEX idx_b ON t (b);""")
    monkeypatch.setattr(Database, "read_rows", lambda *a: pytest.fail("recorrido completo"))
    assert run(db, "SELECT id FROM t WHERE s = 'á|\"x\nsegunda línea'").rows == [[1]]
    assert run(db, "SELECT id FROM t WHERE s = '\\N'").rows == [[2]]
    assert run(db, "SELECT id FROM t WHERE s = ''").rows == [[3]]
    assert run(db, "SELECT id FROM t WHERE d >= '2026-02-01'").rows == [[2]]
    assert run(db, "SELECT id FROM t WHERE f > -3 AND f < 0").rows == [[1]]
    assert run(db, "SELECT id FROM t WHERE b = FALSE").rows == [[2]]


def test_composite_index_and_constraint_survives_drop_index(db):
    run(db, "CREATE TABLE t (a INT PRIMARY KEY, b INT); INSERT INTO t VALUES (1, 2); "
            "CREATE UNIQUE INDEX pair ON t (a,b)")
    assert (db.path / "pair.idx").read_text() == "[1, 2]|1\n"
    run(db, "DROP INDEX pk_t")
    with pytest.raises(ExecutionError):
        run(db, "INSERT INTO t VALUES (1, 3)")


def test_offsets_rebuilt_after_variable_length_update_and_delete(db, monkeypatch):
    run(db, "CREATE TABLE t (id INT, s VARCHAR(100)); "
            "INSERT INTO t VALUES (1, 'a'), (2, 'b'), (3, 'c'); CREATE INDEX idx ON t (id)")
    run(db, "UPDATE t SET s = 'texto largo, con \"comillas\"\ny acentos: áé' WHERE id = 1")
    run(db, "DELETE FROM t WHERE id = 2; INSERT INTO t VALUES (4, 'final')")
    monkeypatch.setattr(Database, "read_rows", lambda *a: pytest.fail("recorrido completo"))
    result = run(Database(db.path), "SELECT s FROM t WHERE id >= 3 ORDER BY id")
    assert result.rows == [["c"], ["final"]]
    assert run(db, "SELECT s FROM t WHERE id = 1").rows == [['texto largo, con "comillas"\ny acentos: áé']]


def test_randomized_index_matches_scan_after_mutations(db):
    rng = random.Random(427)
    run(db, "CREATE TABLE t (id INT, n INT)")
    values = [(i, rng.choice([None, -5, 0, 2, 10])) for i in range(60)]
    sql_values = ",".join(f"({i}, {'NULL' if n is None else n})" for i, n in values)
    run(db, "INSERT INTO t VALUES " + sql_values)
    predicates = [f"n {op} {v}" for op in ["=", "<", "<=", ">", ">="] for v in [-5, -1, 0, 2, 11]]
    for mutation in [None, "UPDATE t SET n = n + 1 WHERE id < 20", "DELETE FROM t WHERE id >= 40"]:
        if mutation:
            run(db, "CREATE INDEX idx ON t (n)")
            run(db, mutation)
            run(db, "DROP INDEX idx")
        expected = [run(db, f"SELECT id,n FROM t WHERE {p} ORDER BY id").rows for p in predicates]
        run(db, "CREATE INDEX idx ON t (n)")
        for predicate, rows in zip(predicates, expected):
            result = run(Database(db.path), f"SELECT id,n FROM t WHERE {predicate} ORDER BY id")
            assert result.rows == rows
            assert result.plan == "índice idx"
        run(db, "DROP INDEX idx")
