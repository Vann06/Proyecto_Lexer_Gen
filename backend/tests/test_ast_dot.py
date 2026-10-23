"""AST -> DOT para la pestaña AST del IDE."""

from dbms.ast_dot import ast_to_dot
from dbms.builder import build
from dbms.frontend import parse


def dot(sql: str) -> str:
    return ast_to_dot(build(parse(sql)))


def test_estructura_basica():
    d = dot("SELECT nombre FROM estudiantes WHERE edad >= 18;")
    assert d.startswith("digraph AST {") and d.endswith("}")
    assert 'label="Script"' in d
    assert 'label="Select"' in d
    assert 'label="where"' in d and 'label="from_table"' in d
    assert "op: >=" in d and "value: 18" in d


def test_subconsultas_y_aristas_rotuladas():
    d = dot("SELECT 1 FROM t WHERE a NOT IN (SELECT b FROM u) AND NOT EXISTS (SELECT 1 FROM v)"
            " AND c = (SELECT MAX(c) FROM w);")
    assert "InSubquery\\nnegated" in d and "Exists\\nnegated" in d and "ScalarSubquery" in d
    assert 'label="select"' in d and 'label="operand"' in d


def test_oculta_posiciones_y_muestra_listas():
    d = dot("INSERT INTO t (a, b) VALUES (1, FALSE); CREATE TABLE p (n VARCHAR(5));")
    assert "Pos" not in d and "line" not in d
    assert "columns: a, b" in d
    assert 'label="rows[0][1]"' in d and "value: FALSE" in d
    assert "type: VARCHAR(5)" in d
    assert '-> a' in d and 'label="2"' in d       # una arista por sentencia


def test_script_vacio():
    assert dot("").count("->") == 0
