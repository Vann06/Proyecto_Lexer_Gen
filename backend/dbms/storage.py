"""Almacenamiento en archivos de texto: el único módulo que toca el disco.

    data/                       DataDir: una carpeta por base de datos
    └── principal/              Database
        ├── metadata.txt        esquema: tablas, columnas, restricciones, índices
        ├── estudiantes.csv     filas de una tabla
        └── ...

metadata.txt (lo escribe `dump_metadata`, lo lee `parse_metadata`):

    TABLE estudiantes
      COLUMN id INT NOT NULL
      COLUMN nombre VARCHAR(50)
      PRIMARY KEY pk_estudiantes (id)
      FOREIGN KEY fk_x (curso_id) REFERENCES cursos (id)
      CHECK ck_estudiantes_1 (edad >= 0)
      NEXT_ROWID 4
    INDEX idx_edad ON estudiantes (edad)

Los nombres van tal cual si son identificadores simples en minúsculas y
entre comillas dobles si no ("Mi Tabla").

<tabla>.csv: encabezado `_rowid,col1,col2,...` y una fila por registro. El
_rowid es un número único por fila que nunca cambia ni se reutiliza (los
índices apuntan a él); NULL se escribe \\N (ver `dbms.sqltypes`).

Toda escritura es atómica: se escribe un .tmp y se reemplaza el original,
así un fallo a la mitad no deja un archivo cortado.

Límite conocido: en Windows el sistema de archivos no distingue mayúsculas,
así que dos tablas "Clientes" y clientes compartirían archivo.
"""

from __future__ import annotations

import csv
import io
import os
import re
import shutil
from dataclasses import dataclass
from pathlib import Path

from dbms import sqltypes
from dbms.catalog import (CHECK, FOREIGN_KEY, PRIMARY_KEY, UNIQUE, Catalog, ColumnSchema,
                          ConstraintSchema, IndexSchema, TableSchema)

METADATA_FILE = "metadata.txt"
# No "default": DEFAULT es palabra reservada y `USE default` no parsearía.
DEFAULT_DATABASE = "principal"
ROWID = "_rowid"


class StorageError(Exception):
    """Un archivo de la base de datos no se pudo leer o escribir."""


@dataclass
class Row:
    rowid: int
    values: dict[str, object]       # columna -> valor en memoria (ver dbms.sqltypes)


# ───────────────────────────── Nombres ─────────────────────────────

_BARE = re.compile(r"[a-z_][a-z0-9_]*")
_FORBIDDEN = re.compile(r'[\\/:*?"<>|\x00-\x1f]')


def _quote(name: str) -> str:
    return name if _BARE.fullmatch(name) else '"' + name.replace('"', '""') + '"'


def _unquote(token: str) -> str:
    return token[1:-1].replace('""', '"') if token.startswith('"') else token


def _check_filename(name: str) -> str:
    if not name or name in (".", "..") or _FORBIDDEN.search(name) or name != name.strip():
        raise StorageError(f"'{name}' no se puede usar como nombre de archivo")
    return name


# ───────────────────────────── metadata.txt ─────────────────────────────

_NAME = r'(?:[A-Za-z_][A-Za-z0-9_]*|"(?:[^"]|"")+")'
_COLS = rf'\(\s*{_NAME}(?:\s*,\s*{_NAME})*\s*\)'
_LINE = {
    "TABLE": re.compile(rf"TABLE\s+({_NAME})"),
    "COLUMN": re.compile(rf"COLUMN\s+({_NAME})\s+([A-Z]+(?:\(\d+\))?)(\s+NOT\s+NULL)?"),
    "PRIMARY": re.compile(rf"PRIMARY\s+KEY\s+({_NAME})\s+({_COLS})"),
    "UNIQUE": re.compile(rf"UNIQUE\s+({_NAME})\s+({_COLS})"),
    "FOREIGN": re.compile(rf"FOREIGN\s+KEY\s+({_NAME})\s+({_COLS})\s+REFERENCES\s+({_NAME})(?:\s+({_COLS}))?"),
    "CHECK": re.compile(rf"CHECK\s+({_NAME})\s+\((.*)\)"),
    "NEXT_ROWID": re.compile(r"NEXT_ROWID\s+(\d+)"),
    "INDEX": re.compile(rf"INDEX\s+({_NAME})\s+ON\s+({_NAME})\s+({_COLS})((?:\s+(?:UNIQUE|IMPLICIT))*)"),
}


def _cols(group: str) -> list[str]:
    return [_unquote(n) for n in re.findall(_NAME, group)]


def _fmt_cols(cols: list[str]) -> str:
    return "(" + ", ".join(_quote(c) for c in cols) + ")"


def dump_metadata(catalog: Catalog) -> str:
    out = ["# Esquema de la base de datos. Lo escribe el DBMS: editarlo a mano puede",
           "# dejar la base inconsistente con sus archivos .csv.", ""]
    for t in catalog.tables.values():
        out.append(f"TABLE {_quote(t.name)}")
        for c in t.columns:
            out.append(f"  COLUMN {_quote(c.name)} {c.type}" + (" NOT NULL" if c.not_null else ""))
        for k in t.constraints:
            if k.kind == CHECK:
                out.append(f"  CHECK {_quote(k.name)} ({k.check})")   # ya en una línea (table_from_ast)
            elif k.kind == FOREIGN_KEY:
                ref = f" {_fmt_cols(k.ref_columns)}" if k.ref_columns else ""
                out.append(f"  FOREIGN KEY {_quote(k.name)} {_fmt_cols(k.columns)} REFERENCES {_quote(k.ref_table)}{ref}")
            else:
                out.append(f"  {k.kind} {_quote(k.name)} {_fmt_cols(k.columns)}")
        out.append(f"  NEXT_ROWID {t.next_rowid}")
        out.append("")
    for i in catalog.indexes.values():
        flags = (" UNIQUE" if i.unique else "") + (" IMPLICIT" if i.implicit else "")
        out.append(f"INDEX {_quote(i.name)} ON {_quote(i.table)} {_fmt_cols(i.columns)}{flags}")
    return "\n".join(out).rstrip("\n") + "\n"


def parse_metadata(text: str, source: str = METADATA_FILE) -> Catalog:
    catalog = Catalog()
    table: TableSchema | None = None
    for lineno, raw in enumerate(text.splitlines(), start=1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue

        def fail(msg: str):
            raise StorageError(f"{source}:{lineno}: {msg}: {line!r}")

        keyword = line.split(None, 1)[0]
        pattern = _LINE.get(keyword)
        m = pattern.fullmatch(line) if pattern else None
        if m is None:
            fail("línea no válida")
        if keyword == "TABLE":
            table = TableSchema(_unquote(m.group(1)), [])
            if table.name in catalog.tables:
                fail("tabla repetida")
            catalog.tables[table.name] = table
            continue
        if keyword == "INDEX":
            flags = m.group(4).split()
            catalog.indexes[_unquote(m.group(1))] = IndexSchema(
                _unquote(m.group(1)), _unquote(m.group(2)), _cols(m.group(3)),
                unique="UNIQUE" in flags, implicit="IMPLICIT" in flags)
            continue
        if table is None:
            fail(f"{keyword} fuera de un bloque TABLE")
        if keyword == "COLUMN":
            try:
                dtype = sqltypes.parse_type(m.group(2))
            except ValueError as e:
                fail(str(e))
            table.columns.append(ColumnSchema(_unquote(m.group(1)), dtype, m.group(3) is not None))
        elif keyword == "PRIMARY":
            table.constraints.append(ConstraintSchema(PRIMARY_KEY, _unquote(m.group(1)), _cols(m.group(2))))
        elif keyword == "UNIQUE":
            table.constraints.append(ConstraintSchema(UNIQUE, _unquote(m.group(1)), _cols(m.group(2))))
        elif keyword == "FOREIGN":
            table.constraints.append(ConstraintSchema(
                FOREIGN_KEY, _unquote(m.group(1)), _cols(m.group(2)), _unquote(m.group(3)),
                _cols(m.group(4)) if m.group(4) else []))
        elif keyword == "CHECK":
            table.constraints.append(ConstraintSchema(CHECK, _unquote(m.group(1)), check=m.group(2)))
        elif keyword == "NEXT_ROWID":
            table.next_rowid = int(m.group(1))
    return catalog


# ───────────────────────────── Escritura atómica ─────────────────────────────

def _atomic_write(path: Path, text: str) -> None:
    tmp = path.with_name(path.name + ".tmp")
    try:
        with open(tmp, "w", encoding="utf-8", newline="") as f:
            f.write(text)
        os.replace(tmp, path)
    except OSError as e:
        tmp.unlink(missing_ok=True)
        raise StorageError(f"no se pudo escribir {path.name}: {e}") from e


# ───────────────────────────── Una base de datos ─────────────────────────────

class Database:
    """Una carpeta con su metadata.txt y un .csv por tabla."""

    def __init__(self, path: Path):
        self.path = Path(path)
        self.name = self.path.name

    @property
    def metadata_path(self) -> Path:
        return self.path / METADATA_FILE

    def table_path(self, table: str) -> Path:
        return self.path / f"{_check_filename(table)}.csv"

    # Catálogo
    def load_catalog(self) -> Catalog:
        if not self.metadata_path.is_file():
            return Catalog()
        return parse_metadata(self.metadata_path.read_text(encoding="utf-8"), f"{self.name}/{METADATA_FILE}")

    def save_catalog(self, catalog: Catalog) -> None:
        _atomic_write(self.metadata_path, dump_metadata(catalog))

    # Archivos de tabla
    def create_table_file(self, table: TableSchema) -> None:
        path = self.table_path(table.name)
        if path.exists():
            raise StorageError(f"ya existe el archivo {path.name}")
        self.write_rows(table, [])

    def drop_table_file(self, name: str) -> None:
        self.table_path(name).unlink(missing_ok=True)

    def read_rows(self, table: TableSchema) -> list[Row]:
        path = self.table_path(table.name)
        if not path.is_file():
            raise StorageError(f"falta el archivo {path.name} de la tabla '{table.name}'")
        expected = [ROWID, *table.column_names]
        rows: list[Row] = []
        with open(path, encoding="utf-8", newline="") as f:
            reader = csv.reader(f)
            header = next(reader, None)
            if header != expected:
                raise StorageError(f"{path.name}: el encabezado {header} no coincide con el esquema {expected}")
            for record in reader:
                where = f"{path.name}:{reader.line_num}"
                if len(record) != len(expected):
                    raise StorageError(f"{where}: se esperaban {len(expected)} campos y hay {len(record)}")
                try:
                    values = {c.name: sqltypes.from_text(cell, c.type)
                              for c, cell in zip(table.columns, record[1:])}
                    rows.append(Row(int(record[0]), values))
                except ValueError as e:
                    raise StorageError(f"{where}: valor inválido ({e})") from e
        return rows

    def write_rows(self, table: TableSchema, rows: list[Row]) -> None:
        buf = io.StringIO()
        writer = csv.writer(buf, lineterminator="\n")
        writer.writerow([ROWID, *table.column_names])
        for row in rows:
            writer.writerow([row.rowid, *(sqltypes.to_text(row.values.get(c.name), c.type)
                                          for c in table.columns)])
        _atomic_write(self.table_path(table.name), buf.getvalue())


# ───────────────────────────── El directorio de datos ─────────────────────────────

class DataDir:
    """La carpeta raíz: una subcarpeta por base de datos."""

    def __init__(self, root: Path):
        self.root = Path(root)

    def _path(self, name: str) -> Path:
        return self.root / _check_filename(name)

    def databases(self) -> list[str]:
        if not self.root.is_dir():
            return []
        return sorted(p.name for p in self.root.iterdir() if p.is_dir())

    def exists(self, name: str) -> bool:
        return self._path(name).is_dir()

    def create(self, name: str) -> Database:
        path = self._path(name)
        if path.exists():
            raise StorageError(f"la base de datos '{name}' ya existe")
        path.mkdir(parents=True)
        db = Database(path)
        db.save_catalog(Catalog())
        return db

    def open(self, name: str = DEFAULT_DATABASE, create: bool = False) -> Database:
        """Abre una base existente; con `create=True` la crea si falta (así
        arranca la base `principal`)."""
        if not self.exists(name):
            if not create:
                raise StorageError(f"la base de datos '{name}' no existe")
            return self.create(name)
        return Database(self._path(name))

    def drop(self, name: str) -> None:
        path = self._path(name)
        if not path.is_dir():
            raise StorageError(f"la base de datos '{name}' no existe")
        shutil.rmtree(path)
