from itertools import product

import pytest

from dbms import ast
from dbms.builder import build
from dbms.eval import Environment, evaluate
from dbms.frontend import parse
from dbms.result import ExecutionError, Result


def expr(sql):
    parsed = parse("SELECT " + sql)
    assert parsed.ok, parsed.problems
    return build(parsed)[0].body.items[0].expr


@pytest.mark.parametrize("a,b", list(product([True, False, None], repeat=2)))
def test_three_valued_truth_tables(a, b):
    literal = {True: "TRUE", False: "FALSE", None: "NULL"}
    expected_and = False if False in (a, b) else None if None in (a, b) else True
    expected_or = True if True in (a, b) else None if None in (a, b) else False
    assert evaluate(expr(f"{literal[a]} AND {literal[b]}")) is expected_and
    assert evaluate(expr(f"{literal[a]} OR {literal[b]}")) is expected_or


@pytest.mark.parametrize("sql,expected", [
    ("NOT NULL", None), ("NOT FALSE", True), ("NOT TRUE", False),
    ("NULL = 1", None), ("NULL <> NULL", None), ("NULL + 2", None),
    ("1 + 2 * 3", 7), ("-7 / 2", -3), ("-7 % 2", -1), ("7 / 2.0", 3.5),
    ("'a' || 'b'", "ab"), ("'a' || NULL", None),
    ("2 IN (1, NULL, 2)", True), ("2 NOT IN (1, NULL)", None),
    ("NULL IN (1, 2)", None), ("2 NOT IN (1, 3)", True),
    ("NULL IS NULL", True), ("NULL IS NOT NULL", False),
    ("1 BETWEEN NULL AND 0", False), ("2 BETWEEN 1 AND 3", True),
    ("2 NOT BETWEEN 1 AND 3", False), ("'abc' LIKE 'a_c'", True),
    ("'a.c' LIKE 'a.c'", True), ("'abc' LIKE 'a.c'", False),
    ("NULL LIKE '%'", None), ("FALSE AND 1 / 0 = 0", False),
    ("TRUE OR 1 / 0 = 0", True),
])
def test_expressions(sql, expected):
    assert evaluate(expr(sql)) == expected


def test_environment_scope_shadowing_and_ambiguity():
    env = Environment([{"local_t": {"id": 2}}, {"external_t": {"id": 1, "x": 3}}])
    assert evaluate(expr("id"), env) == 2
    assert evaluate(expr("external_t.id"), env) == 1
    assert evaluate(expr("x"), env) == 3
    assert evaluate(expr("id"), [{"t": {"id": 4}}]) == 4
    with pytest.raises(ExecutionError, match="ambigua"):
        evaluate(expr("id"), Environment([{"a": {"id": 1}, "b": {"id": 2}}]))
    with pytest.raises(ExecutionError, match="no disponible"):
        evaluate(expr("t.x"), Environment([{"t": {"id": 1}}, {"t": {"x": 2}}]))


def test_subquery_extension_receives_current_environment():
    calls = []
    def runner(select, env):
        calls.append(env.resolve(ast.ColumnRef(None, "id", ast.Pos(1, 1))))
        return Result(["v"], [[2], [None]])
    env = Environment(subquery_runner=runner).push({"t": {"id": 2}})
    assert evaluate(expr("id IN (SELECT 2)"), env) is True
    assert evaluate(expr("3 NOT IN (SELECT 2)"), env) is None
    assert evaluate(expr("EXISTS (SELECT 2)"), env) is True
    with pytest.raises(ExecutionError) as error:
        evaluate(expr("(SELECT 2)"), env)
    assert error.value.code == "EXE003"
    assert calls == [2, 2, 2, 2]


def test_missing_extensions_and_arithmetic_errors_are_explicit():
    for sql, code in [("1 / 0", "EXE004"), ("1 % 0", "EXE004"),
                      ("(SELECT 1)", "EXE099"), ("COUNT(*)", "EXE099")]:
        with pytest.raises(ExecutionError) as error:
            evaluate(expr(sql))
        assert error.value.code == code
