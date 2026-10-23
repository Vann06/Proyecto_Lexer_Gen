"""Fase B: catálogo y conversión de CREATE TABLE."""

import pytest

from dbms import sqltypes as T
from dbms.builder import build
from dbms.catalog import (CHECK, FOREIGN_KEY, PRIMARY_KEY, UNIQUE, Catalog, IndexSchema,
                          table_from_ast)
from dbms.frontend import parse


def create(sql: str, catalog: Catalog | None = None):
    return table_from_ast(build(parse(sql))[0].body, catalog)


def test_columnas_y_tipos():
    t = create("CREATE TABLE e (id INT, nombre VARCHAR(50) NOT NULL, nota FLOAT, ok BOOLEAN, alta DATE)")
    assert t.column_names == ["id", "nombre", "nota", "ok", "alta"]
    assert t.column("nombre").type == T.varchar(50) and t.column("nombre").not_null
    assert not t.column("nota").not_null
    assert t.column("nada") is None
    assert t.next_rowid == 1


def test_pk_hace_not_null_y_nombres_generados():
    t = create("CREATE TABLE e (id INT PRIMARY KEY, carne VARCHAR(10) UNIQUE, a INT, b INT, "
               "UNIQUE (a, b), CHECK (a > 0), CHECK (b > 0))")
    assert t.column("id").not_null
    assert [(c.kind, c.name, c.columns) for c in t.constraints] == [
        (PRIMARY_KEY, "pk_e", ["id"]),
        (UNIQUE, "uq_e_carne", ["carne"]),
        (UNIQUE, "uq_e_a_b", ["a", "b"]),
        (CHECK, "ck_e_1", []),
        (CHECK, "ck_e_2", []),
    ]
    assert t.constraints[3].check == "a > 0"
    assert t.primary_key.columns == ["id"]


def test_pk_compuesta_y_nombres_del_usuario():
    t = create("CREATE TABLE i (e INT, c INT, CONSTRAINT pk_insc PRIMARY KEY (e, c), "
               "CONSTRAINT fk_e FOREIGN KEY (e) REFERENCES estudiantes (id))")
    assert t.primary_key.name == "pk_insc" and t.primary_key.columns == ["e", "c"]
    assert t.column("e").not_null and t.column("c").not_null
    fk = t.constraints_of(FOREIGN_KEY)[0]
    assert (fk.name, fk.columns, fk.ref_table, fk.ref_columns) == ("fk_e", ["e"], "estudiantes", ["id"])


def test_nombres_generados_no_chocan():
    t = create("CREATE TABLE t (a INT UNIQUE, UNIQUE (a))")
    assert [c.name for c in t.constraints] == ["uq_t_a", "uq_t_a_2"]


def test_fk_sin_columnas_usa_la_pk_referenciada():
    catalog = Catalog()
    catalog.add_table(create("CREATE TABLE cursos (codigo INT PRIMARY KEY)"))
    t = create("CREATE TABLE i (c INT REFERENCES cursos, x INT REFERENCES nadie)", catalog)
    fks = t.constraints_of(FOREIGN_KEY)
    assert fks[0].ref_columns == ["codigo"]
    assert fks[1].ref_columns == []          # tabla inexistente: lo reporta la semántica


def test_fk_a_la_propia_tabla_aunque_la_pk_venga_despues():
    t = create("CREATE TABLE emp (jefe INT REFERENCES emp, id INT, PRIMARY KEY (id))")
    assert t.constraints_of(FOREIGN_KEY)[0].ref_columns == ["id"]


def test_catalogo_tablas_e_indices():
    c = Catalog()
    c.add_table(create("CREATE TABLE e (id INT, edad INT)"))
    c.add_table(create("CREATE TABLE o (id INT)"))
    with pytest.raises(KeyError):
        c.add_table(create("CREATE TABLE e (x INT)"))
    c.add_index(IndexSchema("idx_edad", "e", ["edad"]))
    c.add_index(IndexSchema("idx_comp", "e", ["id", "edad"]))
    c.add_index(IndexSchema("idx_o", "o", ["id"]))
    with pytest.raises(KeyError):
        c.add_index(IndexSchema("idx_edad", "o", ["id"]))
    assert c.index_for("e", "edad").name == "idx_edad"
    assert c.index_for("e", "id") is None    # idx_comp es compuesto: no sirve para `id = v`
    dropped = c.drop_table("e")
    assert sorted(i.name for i in dropped) == ["idx_comp", "idx_edad"]
    assert list(c.indexes) == ["idx_o"] and c.table("e") is None
    assert c.drop_index("idx_o").table == "o" and not c.indexes
