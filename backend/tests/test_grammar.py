"""Fase 1-2: la gramática acepta el SQL soportado y reporta con código,
línea y columna lo que no lo es."""

import pytest

from dbms.frontend import parse, tokens_to_json, tree_to_dot

VALID = [
    "CREATE DATABASE tienda",
    "create database if not exists tienda",
    "DROP DATABASE IF EXISTS tienda",
    "ALTER DATABASE tienda RENAME TO shop",
    "USE tienda",
    "USE DATABASE tienda",
    "SHOW DATABASES",
    "SHOW TABLES",
    "SHOW COLUMNS FROM cliente",
    "DESCRIBE cliente",
    "CREATE TABLE t (id INT PRIMARY KEY, n VARCHAR(20) NOT NULL UNIQUE, p FLOAT DEFAULT -1.5, "
    "c CHAR(2) NULL, d DATE, b BOOLEAN DEFAULT TRUE, CHECK (p >= 0))",
    "CREATE TABLE p (id INT, cid INT REFERENCES cliente(id), CONSTRAINT pk_p PRIMARY KEY (id), "
    "CONSTRAINT fk_c FOREIGN KEY (cid) REFERENCES cliente (id), UNIQUE (id, cid))",
    "DROP TABLE IF EXISTS t",
    "ALTER TABLE t RENAME TO u",
    "ALTER TABLE t RENAME COLUMN a TO b",
    "ALTER TABLE t ADD COLUMN edad INT NOT NULL DEFAULT 0, DROP COLUMN x",
    "ALTER TABLE t ADD CONSTRAINT ck CHECK (edad > 0)",
    "ALTER TABLE t DROP CONSTRAINT ck",
    "ALTER TABLE t ALTER COLUMN edad TYPE FLOAT",
    "ALTER TABLE t ALTER COLUMN edad SET DATA TYPE VARCHAR(3)",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx ON t (a, b)",
    "DROP INDEX idx",
    "INSERT INTO t VALUES (1, 'a'), (2, 'O''Brien')",
    "INSERT INTO t (a, b) VALUES (NULL, -3)",
    "UPDATE t SET a = a + 1, b = b || '!' WHERE id IN (1, 2, 3)",
    "DELETE FROM t",
    "DELETE FROM t WHERE a IS NOT NULL AND NOT b LIKE 'x%'",
    "SELECT * FROM t",
    "SELECT 1 + 2 * 3",
    "SELECT DISTINCT t.*, a AS x, b y FROM t",
    "SELECT c.n, COUNT(*), SUM(p.total) AS s FROM cliente c "
    "INNER JOIN pedido p ON p.cid = c.id LEFT OUTER JOIN x ON x.id = c.id "
    "RIGHT JOIN y ON y.id = c.id CROSS JOIN z "
    "WHERE c.id BETWEEN 1 AND 5 AND c.n NOT IN ('a') "
    "GROUP BY c.n HAVING COUNT(DISTINCT p.id) > 1 ORDER BY s DESC, c.n LIMIT 10 OFFSET 5",
    'SELECT "Mi Columna", `otra` FROM "Tabla"',
    "SELECT date, type FROM t WHERE date > '2024-01-01'",
    "-- comentario\nSELECT /* en línea */ 1",
]


@pytest.mark.parametrize("sql", VALID)
def test_acepta(sql):
    result = parse(sql + ";")
    assert result.ok, [p.to_dict() for p in result.problems]


def test_script_con_varias_sentencias_y_punto_y_coma_opcional():
    result = parse("USE a; SELECT 1;; SELECT 2")
    assert result.ok
    assert len(result.tree.sqlStmt()) == 3


def test_script_vacio():
    assert parse("").ok
    assert parse("  -- nada\n").ok


@pytest.mark.parametrize("sql, code, line, col", [
    ("SELECT FROM t;", "SYN002", 1, 8),
    ("CREATE TABLE t (id INT,);", "SYN001", 1, 24),
    ("UPDATE t SET a 1;", "SYN003", 1, 16),
    ("CREATE TABLA x;", "SYN004", 1, 8),
    ("SELECT @ FROM t;", "LEX001", 1, 8),
    ("SELECT 1;\nSELECT * FROM t WHERE a = 'abc", "LEX002", 2, 27),
    ("SELECT a FROM t WHERE", "SYN001", 1, 22),
])
def test_rechaza_con_codigo_y_posicion(sql, code, line, col):
    result = parse(sql)
    assert not result.ok
    first = result.problems[0]
    assert (first.code, first.line, first.col) == (code, line, col), first.to_dict()


def test_cadena_sin_cerrar_se_reporta_una_sola_vez():
    problems = parse("SELECT 'abc").problems
    assert [p.code for p in problems] == ["LEX002"]


def test_mensajes_en_espanol_sin_ruido():
    msg = parse("CREATE TABLE t (id INT,);").problems[0].msg
    assert "se esperaba" in msg and "identificador" in msg
    assert "QUOTED_IDENTIFIER" not in msg and "'TYPE'" not in msg
    assert parse("INSERT INTO t VALUES (1 2);").problems[0].msg.endswith("uno de: ')', ','")


def test_tokens_para_el_ide():
    toks = tokens_to_json(parse("SELECT a\nFROM t;"))
    assert toks[0] == {"kind": "SELECT", "lexeme": "SELECT", "line": 1, "col": 1}
    assert toks[2] == {"kind": "FROM", "lexeme": "FROM", "line": 2, "col": 1}
    assert toks[-1]["kind"] == "SEMI"


def test_palabras_clave_sin_distinguir_mayusculas():
    kinds = [t["kind"] for t in tokens_to_json(parse("select SeLeCt SELECT"))]
    assert kinds == ["SELECT"] * 3


def test_dot_del_arbol():
    dot = tree_to_dot(parse("SELECT a FROM t WHERE a = 1;"))
    assert dot.startswith("digraph ParseTree {") and dot.endswith("}")
    assert "predicate · comparePred" in dot
    assert '"script"' in dot
