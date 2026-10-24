import pytest

from test_executor import db, run


def test_order_alias_expression_multiple_keys_and_nulls(db):
    run(db, "CREATE TABLE t (id INT, n INT); INSERT INTO t VALUES (1, 2), (2, NULL), (3, 2), (4, 1)")
    result = run(db, "SELECT id, n + 1 AS valor FROM t ORDER BY valor DESC, id DESC")
    assert result.columns == ["id", "valor"]
    assert result.rows == [[3, 3], [1, 3], [4, 2], [2, None]]
    assert run(db, "SELECT id FROM t ORDER BY n ASC, id DESC").rows == [[4], [3], [1], [2]]
    assert run(db, "SELECT id FROM t ORDER BY -id").rows == [[4], [3], [2], [1]]


def test_distinct_limit_offset_star_and_no_from(db):
    run(db, "CREATE TABLE t (n INT); INSERT INTO t VALUES (2), (NULL), (2), (1)")
    assert run(db, "SELECT DISTINCT n FROM t ORDER BY n LIMIT 1 OFFSET 1").rows == [[2]]
    assert run(db, "SELECT x.* FROM t x LIMIT 0").columns == ["n"]
    assert run(db, "SELECT * FROM t LIMIT 0").rows == []
    assert run(db, "SELECT 1 + 2 AS suma WHERE TRUE").rows == [[3]]
    assert run(db, "SELECT 1 WHERE NULL = 1").rows == []


def test_dates_and_filters(db):
    run(db, "CREATE TABLE t (id INT, d DATE, activo BOOLEAN, s VARCHAR(20)); "
            "INSERT INTO t VALUES (1, '2026-01-01', TRUE, 'Ana'), "
            "(2, '2026-09-01', FALSE, 'Luis'), (3, NULL, NULL, NULL)")
    assert run(db, "SELECT id FROM t WHERE d >= '2026-02-01'").rows == [[2]]
    assert run(db, "SELECT id FROM t WHERE d IN ('2026-01-01', NULL)").rows == [[1]]
    assert run(db, "SELECT id FROM t WHERE activo AND s LIKE 'A%'").rows == [[1]]
    assert run(db, "SELECT id FROM t WHERE d BETWEEN '2026-01-01' AND '2026-02-01'").rows == [[1]]


@pytest.mark.parametrize("query", ["SELECT COUNT(*) FROM t", "SELECT n FROM t GROUP BY n",
                                  "SELECT a.n FROM t a INNER JOIN t b ON a.n = b.n"])
def test_unconnected_d3_is_never_silently_ignored(db, query):
    from dbms.result import ExecutionError
    run(db, "CREATE TABLE t (n INT)")
    with pytest.raises(ExecutionError) as error:
        run(db, query)
    assert error.value.code == "EXE099"


def test_join_module_extension_runs_before_where(db, monkeypatch):
    import sys
    from types import SimpleNamespace
    from dbms.eval import Environment, evaluate
    run(db, "CREATE TABLE t (n INT); INSERT INTO t VALUES (1), (2)")
    def rows(stmt, database, envs):
        for env in envs:
            for value in (1, 2):
                combined = Environment([{**env.scopes[0], "b": {"n": value}}, *env.scopes[1:]])
                if evaluate(stmt.joins[0].on, combined) is True:
                    yield combined
    monkeypatch.setitem(sys.modules, "dbms.join", SimpleNamespace(rows=rows))
    result = run(db, "SELECT a.n, b.n FROM t a INNER JOIN t b ON a.n = b.n WHERE b.n > 1")
    assert result.rows == [[2, 2]]


def test_group_module_extension_and_having_order_projection(db, monkeypatch):
    import sys
    from types import SimpleNamespace
    from dbms.eval import Environment
    run(db, "CREATE TABLE t (n INT); INSERT INTO t VALUES (1), (2), (3)")
    def group(stmt, envs):
        assert len(envs) == 2  # WHERE ya se aplicó
        return [Environment(aggregate_runner=lambda call, env: len(envs))]
    monkeypatch.setitem(sys.modules, "dbms.aggregate", SimpleNamespace(group=group))
    result = run(db, "SELECT COUNT(*) AS total FROM t WHERE n > 1 HAVING COUNT(*) > 1 ORDER BY total")
    assert result.columns == ["total"]
    assert result.rows == [[2]]
