"""Motor real: SQL -> AST -> semántica -> ejecución -> archivos en tmp_path."""
from datetime import date
import json

import pytest

from dbms.builder import build
from dbms.executor import execute
from dbms.frontend import parse
from dbms.result import ExecutionError, Result
from dbms.semantic import check
from dbms.storage import DataDir, Database


def run(db, sql):
    parsed = parse(sql)
    assert parsed.ok, parsed.problems
    results = []
    for stmt in build(parsed):
        errors = [p for p in check(stmt, db.load_catalog()) if p.level == "err"]
        assert not errors, errors
        results.append(execute(stmt, db))
    return results[-1]


@pytest.fixture
def db(tmp_path):
    return DataDir(tmp_path).create("motor")


def snapshot(db):
    return {p.name: p.read_bytes() for p in db.path.iterdir() if p.is_file()}


def test_create_insert_project_reopen_and_drop(db):
    run(db, """CREATE TABLE estudiantes (id INT PRIMARY KEY, nombre VARCHAR(30) NOT NULL,
               edad INT, nota FLOAT, activo BOOLEAN, ingreso DATE, carne VARCHAR(10) UNIQUE);
               INSERT INTO estudiantes VALUES (1, 'Ana', 19, 85, TRUE, '2026-09-01', 'A1');
               INSERT INTO estudiantes (id, nombre) VALUES (2, 'Luis');""")
    result = run(Database(db.path), "SELECT * FROM estudiantes ORDER BY id")
    assert result.columns == ["id", "nombre", "edad", "nota", "activo", "ingreso", "carne"]
    assert result.rows == [[1, "Ana", 19, 85.0, True, date(2026, 9, 1), "A1"],
                           [2, "Luis", None, None, None, None, None]]
    assert json.loads(json.dumps(result.to_dict()))["rows"][0][5] == "2026-09-01"
    assert "NEXT_ROWID 3" in db.metadata_path.read_text()
    assert "\\N" in db.table_path("estudiantes").read_text()
    assert len(list(db.path.glob("*.idx"))) == 2
    run(db, "CREATE TABLE IF NOT EXISTS estudiantes (x INT)")
    assert run(db, "SELECT nombre FROM estudiantes WHERE edad >= 18").rows == [["Ana"]]
    run(db, "DROP TABLE estudiantes; DROP TABLE IF EXISTS estudiantes")
    assert list(db.path.iterdir()) == [db.metadata_path]
    assert not db.load_catalog().tables
    assert not db.load_catalog().indexes


@pytest.mark.parametrize("sql,code", [
    ("INSERT INTO t VALUES (2, 8), (1, 9)", "EXE001"),
    ("INSERT INTO t VALUES (2, 7)", "EXE001"),
    ("UPDATE t SET id = NULL + 1", "EXE002"),
    ("UPDATE t SET n = 1 / (id - 1)", "EXE004"),
])
def test_failed_mutation_keeps_all_files(db, sql, code):
    run(db, "CREATE TABLE t (id INT PRIMARY KEY, n INT UNIQUE); INSERT INTO t VALUES (1, 7)")
    before = snapshot(db)
    with pytest.raises(ExecutionError) as error:
        run(db, sql)
    assert error.value.code == code
    assert error.value.to_problem().line == 1
    assert snapshot(db) == before


def test_update_simultaneous_delete_and_rowids(db):
    run(db, "CREATE TABLE t (a INT PRIMARY KEY, b INT); INSERT INTO t VALUES (1, 2), (3, 4), (5, NULL)")
    assert run(db, "UPDATE t SET a = b, b = a WHERE a < 4").message == "2 filas afectadas"
    assert run(db, "SELECT * FROM t ORDER BY a").rows == [[2, 1], [4, 3], [5, None]]
    assert run(db, "DELETE FROM t WHERE b IS NULL OR b = 3").message == "2 filas afectadas"
    run(db, "INSERT INTO t VALUES (6, 7)")
    rows = db.read_rows(db.load_catalog().table("t"))
    assert [r.rowid for r in rows] == [1, 4]
    assert db.load_catalog().table("t").next_rowid == 5
    assert run(db, "UPDATE t SET b = 0 WHERE NULL = 1").message == "0 filas afectadas"
    assert run(db, "DELETE FROM t").message == "2 filas afectadas"


def test_composite_constraints_and_nullable_unique(db):
    run(db, "CREATE TABLE t (a INT, b INT, c INT, PRIMARY KEY (a,b), UNIQUE (b,c)); "
            "INSERT INTO t VALUES (1, 2, NULL), (2, 2, NULL), (1, 3, 4)")
    before = snapshot(db)
    with pytest.raises(ExecutionError, match="duplicada"):
        run(db, "INSERT INTO t VALUES (1, 2, 9)")
    with pytest.raises(ExecutionError, match="duplicada"):
        run(db, "INSERT INTO t VALUES (9, 3, 4)")
    assert snapshot(db) == before


def test_runtime_coercion_error_is_atomic(db):
    run(db, "CREATE TABLE t (s VARCHAR(3)); INSERT INTO t VALUES ('abc'), ('a')")
    before = snapshot(db)
    with pytest.raises(ExecutionError) as error:
        run(db, "UPDATE t SET s = s || 'x'")
    assert error.value.code == "EXE004"
    assert snapshot(db) == before


def test_result_defaults_are_independent():
    a, b = Result(), Result()
    a.rows.append([1])
    assert b.to_dict() == {"columns": [], "rows": [], "message": "", "plan": None}


def test_duplicate_inside_batch_and_update_collision_leave_files_unchanged(db):
    run(db, "CREATE TABLE t (id INT PRIMARY KEY); INSERT INTO t VALUES (1), (2)")
    before = snapshot(db)
    for sql in ["INSERT INTO t VALUES (3), (3)", "UPDATE t SET id = 8"]:
        with pytest.raises(ExecutionError) as error:
            run(db, sql)
        assert error.value.code == "EXE001"
        assert snapshot(db) == before
