"""Fase 2: árbol de ANTLR -> AST."""

import pytest

from dbms import ast
from dbms.builder import SyntaxErrorsPresent, build
from dbms.frontend import parse


def one(sql: str):
    statements = build(parse(sql))
    assert len(statements) == 1
    return statements[0].body


def test_statement_conserva_texto_y_posicion():
    stmts = build(parse("USE a;\n  SELECT  1 ;"))
    assert [s.kind for s in stmts] == ["UseDatabase", "Select"]
    assert stmts[1].sql == "SELECT  1"
    assert stmts[1].pos == ast.Pos(2, 3)


def test_no_construye_con_errores_de_sintaxis():
    with pytest.raises(SyntaxErrorsPresent):
        build(parse("SELECT FROM;"))


def test_identificadores_normalizados():
    s = one('SELECT Nombre, "Mixto" FROM Clientes')
    assert s.items[0].expr == ast.ColumnRef(None, "nombre", ast.Pos(1, 8))
    assert s.items[1].expr.column == "Mixto"
    assert s.from_table.name == "clientes"


def test_create_table_unifica_restricciones_de_columna_y_tabla():
    s = one("CREATE TABLE p (id INT PRIMARY KEY, n VARCHAR(10) NOT NULL DEFAULT 'x', "
            "cid INT CONSTRAINT fk1 REFERENCES cliente(id), m FLOAT DEFAULT -2, "
            "CONSTRAINT u1 UNIQUE (n, m), CHECK (m >= 0))")
    assert isinstance(s, ast.CreateTable) and s.name == "p"
    assert [c.name for c in s.columns] == ["id", "n", "cid", "m"]
    assert s.columns[1].type == ast.DataType("VARCHAR", 10)
    assert s.columns[1].not_null and s.columns[1].default.value == "x"
    assert s.columns[3].default == ast.Literal(-2, "int", s.columns[3].default.pos)
    kinds = [(type(c).__name__, c.name, c.columns if not isinstance(c, ast.Check) else None)
             for c in s.constraints]
    assert kinds == [("PrimaryKey", None, ["id"]), ("ForeignKey", "fk1", ["cid"]),
                     ("Unique", "u1", ["n", "m"]), ("Check", None, None)]
    fk = s.constraints[1]
    assert (fk.ref_table, fk.ref_columns) == ("cliente", ["id"])
    assert s.constraints[3].text == "m >= 0"


def test_foreign_key_sin_columnas_referenciadas():
    s = one("CREATE TABLE p (a INT, FOREIGN KEY (a) REFERENCES q)")
    assert s.constraints[0].ref_columns == []


def test_alter_table_acciones():
    s = one("ALTER TABLE t ADD COLUMN e INT UNIQUE, DROP COLUMN x, RENAME COLUMN a TO b, "
            "ADD CONSTRAINT ck CHECK (e > 0), DROP CONSTRAINT ck, ALTER COLUMN e TYPE CHAR(3), RENAME TO u")
    names = [type(a).__name__ for a in s.actions]
    assert names == ["AddColumn", "DropColumn", "RenameColumn", "AddConstraint",
                     "DropConstraint", "AlterColumnType", "RenameTable"]
    assert isinstance(s.actions[0].constraints[0], ast.Unique)
    assert s.actions[5].type == ast.DataType("CHAR", 3)


def test_insert():
    s = one("INSERT INTO t (a, b) VALUES (1, 'O''Brien'), (NULL, TRUE)")
    assert s.columns == ["a", "b"]
    assert [[lit.value for lit in row] for row in s.rows] == [[1, "O'Brien"], [None, True]]


def test_update_y_delete():
    u = one("UPDATE t SET a = a + 1 WHERE id = 3")
    assert u.assignments[0].column == "a"
    assert isinstance(u.assignments[0].value, ast.Binary) and u.assignments[0].value.op == "+"
    assert u.where.op == "="
    d = one("DELETE FROM t")
    assert d.where is None


def test_precedencia_de_expresiones():
    e = one("SELECT 1 FROM t WHERE NOT a = 1 OR b = 2 AND c + 2 * 3 > 4").where
    # OR( NOT(a = 1), AND(b = 2, (c + (2*3)) > 4) )
    assert e.op == "OR"
    assert e.left.op == "NOT" and e.left.operand.op == "="
    assert e.right.op == "AND"
    gt = e.right.right
    assert gt.op == ">" and gt.left.op == "+" and gt.left.right.op == "*"


def test_between_no_se_confunde_con_and():
    e = one("SELECT 1 FROM t WHERE a BETWEEN 1 AND 5 AND b = 2").where
    assert e.op == "AND"
    assert isinstance(e.left, ast.Between) and e.left.high.value == 5


def test_predicados():
    w = one("SELECT 1 FROM t WHERE a NOT IN (1, 2) AND b IS NOT NULL AND c NOT LIKE 'x%' AND d != 1").where
    parts = []
    while isinstance(w, ast.Binary) and w.op == "AND":
        parts.insert(0, w.right)
        w = w.left
    parts.insert(0, w)
    inl, isn, like, neq = parts
    assert isinstance(inl, ast.InList) and inl.negated and len(inl.items) == 2
    assert isinstance(isn, ast.IsNull) and isn.negated
    assert isinstance(like, ast.Like) and like.negated
    assert neq.op == "<>"


def test_select_completo():
    s = one("SELECT DISTINCT c.*, COUNT(*) AS n, SUM(DISTINCT p.total) FROM cliente AS c "
            "LEFT JOIN pedido p ON p.cid = c.id CROSS JOIN z "
            "WHERE c.id > 0 GROUP BY c.id, c.n HAVING COUNT(*) > 1 ORDER BY n DESC, c.n LIMIT 10 OFFSET 2")
    assert s.distinct
    assert s.items[0] == ast.Star("c", s.items[0].pos)
    count = s.items[1]
    assert count.alias == "n" and count.expr.name == "COUNT" and count.expr.star
    total = s.items[2]
    assert total.alias is None and total.text == "SUM(DISTINCT p.total)" and total.expr.distinct
    assert s.from_table.ref_name == "c"
    assert [(j.kind, j.table.ref_name) for j in s.joins] == [("LEFT", "p"), ("CROSS", "z")]
    assert s.joins[1].on is None
    assert len(s.group_by) == 2 and s.having.op == ">"
    assert [o.descending for o in s.order_by] == [True, False]
    assert (s.limit, s.offset) == (10, 2)


def test_select_estrella_y_sin_from():
    assert one("SELECT * FROM t").items == [ast.Star(None, ast.Pos(1, 8))]
    assert one("SELECT 1").from_table is None


def test_indices_y_bases():
    ci = one("CREATE UNIQUE INDEX i ON t (a, b)")
    assert (ci.name, ci.table, ci.columns, ci.unique) == ("i", "t", ["a", "b"], True)
    assert one("ALTER DATABASE a RENAME TO b") == ast.AlterDatabase("a", "b")
    assert one("CREATE DATABASE IF NOT EXISTS x") == ast.CreateDatabase("x", True)
    assert one("DESCRIBE t") == ast.ShowColumns("t")
