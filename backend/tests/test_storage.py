"""Fase B: metadata.txt, archivos .csv y directorios de bases de datos."""

from datetime import date

import pytest

from dbms.builder import build
from dbms.catalog import Catalog, IndexSchema, table_from_ast
from dbms.frontend import parse
from dbms.storage import (DataDir, Database, Row, StorageError, dump_metadata, parse_metadata)


def catalog_from(sql: str) -> Catalog:
    c = Catalog()
    for s in build(parse(sql)):
        c.add_table(table_from_ast(s.body, c))
    return c


SCHEMA = """
CREATE TABLE estudiantes (id INT PRIMARY KEY, nombre VARCHAR(50) NOT NULL, carne VARCHAR(10) UNIQUE,
                          edad INT, promedio FLOAT, activo BOOLEAN, ingreso DATE);
CREATE TABLE inscripciones (
    estudiante_id INT, curso_id INT, nota FLOAT,
    CONSTRAINT pk_insc PRIMARY KEY (estudiante_id, curso_id),
    CONSTRAINT fk_est FOREIGN KEY (estudiante_id) REFERENCES estudiantes,
    FOREIGN KEY (curso_id) REFERENCES cursos,
    CHECK (nota >= 0
           AND nota <= 100));
CREATE TABLE "Mi Tabla" ("Col Rara" INT, x CHAR(3));
"""


@pytest.fixture
def catalog():
    c = catalog_from(SCHEMA)
    c.add_index(IndexSchema("idx_edad", "estudiantes", ["edad"]))
    c.add_index(IndexSchema("pk_estudiantes", "estudiantes", ["id"], unique=True, implicit=True))
    c.tables["estudiantes"].next_rowid = 4
    return c


def test_metadata_formato(catalog):
    text = dump_metadata(catalog)
    assert "TABLE estudiantes\n  COLUMN id INT NOT NULL\n  COLUMN nombre VARCHAR(50) NOT NULL\n" in text
    assert "  PRIMARY KEY pk_estudiantes (id)\n" in text
    assert "  UNIQUE uq_estudiantes_carne (carne)\n" in text
    assert "  NEXT_ROWID 4\n" in text
    assert "  FOREIGN KEY fk_est (estudiante_id) REFERENCES estudiantes (id)\n" in text
    assert "  FOREIGN KEY fk_inscripciones_curso_id (curso_id) REFERENCES cursos\n" in text   # sin resolver
    assert "  CHECK ck_inscripciones_1 (nota >= 0 AND nota <= 100)\n" in text                 # en una línea
    assert 'TABLE "Mi Tabla"\n  COLUMN "Col Rara" INT\n  COLUMN x CHAR(3)\n' in text
    assert "INDEX idx_edad ON estudiantes (edad)\n" in text
    assert "INDEX pk_estudiantes ON estudiantes (id) UNIQUE IMPLICIT\n" in text


def test_metadata_ida_y_vuelta(catalog):
    assert parse_metadata(dump_metadata(catalog)) == catalog


def test_metadata_vacia():
    assert parse_metadata(dump_metadata(Catalog())) == Catalog()


@pytest.mark.parametrize("text, fragment", [
    ("COLUMN id INT", "fuera de un bloque TABLE"),
    ("TABLE t\n  COLUMN id TEXT", "tipo desconocido"),
    ("TABLE t\n  COLUMNA id INT", "línea no válida"),
    ("TABLE t\nTABLE t", "tabla repetida"),
    ("TABLE t\n  NEXT_ROWID x", "línea no válida"),
])
def test_metadata_invalida_con_linea(text, fragment):
    with pytest.raises(StorageError, match=fragment) as e:
        parse_metadata(text)
    assert "metadata.txt:" in str(e.value)


def test_filas_ida_y_vuelta(tmp_path, catalog):
    db = Database(tmp_path)
    t = catalog.table("estudiantes")
    db.create_table_file(t)
    assert db.read_rows(t) == []
    rows = [
        Row(1, {"id": 1, "nombre": "Ana", "carne": "A1", "edad": 20, "promedio": 85.5,
                "activo": True, "ingreso": date(2024, 1, 15)}),
        Row(2, {"id": 2, "nombre": 'O\'Brien, "Pat"', "carne": None, "edad": None, "promedio": None,
                "activo": False, "ingreso": None}),
        Row(5, {"id": 3, "nombre": "\\N", "carne": "", "edad": 0, "promedio": 0.0,
                "activo": None, "ingreso": date(2000, 2, 29)}),
    ]
    db.write_rows(t, rows)
    assert db.read_rows(t) == rows
    text = db.table_path("estudiantes").read_text(encoding="utf-8")
    assert text.splitlines()[0] == "_rowid,id,nombre,carne,edad,promedio,activo,ingreso"
    assert text.splitlines()[2] == '2,2,"O\'Brien, ""Pat""",\\N,\\N,\\N,FALSE,\\N'


def test_columnas_faltantes_se_guardan_como_null(tmp_path, catalog):
    db = Database(tmp_path)
    t = catalog.table("Mi Tabla")
    db.write_rows(t, [Row(1, {"x": "abc"})])
    assert db.read_rows(t) == [Row(1, {"Col Rara": None, "x": "abc"})]
    assert (tmp_path / "Mi Tabla.csv").is_file()


@pytest.mark.parametrize("content, fragment", [
    ("_rowid,otra\n", "no coincide con el esquema"),
    ("_rowid,id,x\n1,2\n", "se esperaban 3 campos"),
    ("_rowid,id,x\n1,dos,abc\n", "estudiantes.csv:2: valor inválido"),
])
def test_csv_corrupto(tmp_path, content, fragment):
    t = catalog_from("CREATE TABLE estudiantes (id INT, x VARCHAR(3))").table("estudiantes")
    (tmp_path / "estudiantes.csv").write_text(content, encoding="utf-8")
    with pytest.raises(StorageError, match=fragment):
        Database(tmp_path).read_rows(t)


def test_falta_el_csv(tmp_path, catalog):
    with pytest.raises(StorageError, match="falta el archivo estudiantes.csv"):
        Database(tmp_path).read_rows(catalog.table("estudiantes"))


def test_crear_y_borrar_archivo_de_tabla(tmp_path, catalog):
    db = Database(tmp_path)
    t = catalog.table("estudiantes")
    db.create_table_file(t)
    with pytest.raises(StorageError, match="ya existe"):
        db.create_table_file(t)
    db.drop_table_file("estudiantes")
    assert not db.table_path("estudiantes").exists()
    db.drop_table_file("estudiantes")        # borrar lo que no existe no falla


def test_escritura_atomica_no_deja_temporales(tmp_path, catalog):
    db = Database(tmp_path)
    db.save_catalog(catalog)
    db.write_rows(catalog.table("estudiantes"), [])
    assert not list(tmp_path.glob("*.tmp"))
    assert db.load_catalog() == catalog


@pytest.mark.parametrize("name", ["a/b", "..", "x:y", "", " t"])
def test_nombres_de_archivo_invalidos(tmp_path, name):
    with pytest.raises(StorageError, match="nombre de archivo"):
        Database(tmp_path).table_path(name)


def test_directorio_de_datos(tmp_path):
    data = DataDir(tmp_path / "data")
    assert data.databases() == []
    with pytest.raises(StorageError, match="no existe"):
        data.open("tienda")
    default = data.open(create=True)                       # crea data/default
    assert default.name == "default" and default.metadata_path.is_file()
    assert default.load_catalog() == Catalog()
    data.create("tienda")
    with pytest.raises(StorageError, match="ya existe"):
        data.create("tienda")
    assert data.databases() == ["default", "tienda"]
    assert data.open("tienda").path == tmp_path / "data" / "tienda"
    data.drop("tienda")
    assert data.databases() == ["default"]
    with pytest.raises(StorageError, match="no existe"):
        data.drop("tienda")
    with pytest.raises(StorageError, match="nombre de archivo"):
        data.create("../fuera")
