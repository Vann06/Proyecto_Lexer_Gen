"""Catálogo: el esquema de una base de datos en memoria.

Es lo que describe `metadata.txt` (ver `dbms.storage`), ya convertido en
objetos: tablas con sus columnas, tipos y restricciones, e índices. La
semántica le pregunta qué existe; el ejecutor lo modifica y lo guarda.

Convenciones:
- Los nombres se comparan tal cual: el builder ya pasó a minúsculas los
  identificadores sin comillas.
- Toda restricción tiene nombre: el del usuario (`CONSTRAINT x`) o uno
  generado (`pk_tabla`, `uq_tabla_col`, ...), para que los errores puedan
  citarla.
- Las columnas de la llave primaria son NOT NULL aunque no se escriba.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

from dbms import ast

PRIMARY_KEY = "PRIMARY KEY"
UNIQUE = "UNIQUE"
FOREIGN_KEY = "FOREIGN KEY"
CHECK = "CHECK"


@dataclass
class ColumnSchema:
    name: str
    type: ast.DataType
    not_null: bool = False


@dataclass
class ConstraintSchema:
    kind: str                       # PRIMARY_KEY | UNIQUE | FOREIGN_KEY | CHECK
    name: str
    columns: list[str] = field(default_factory=list)   # vacío en CHECK
    ref_table: str | None = None    # solo FOREIGN KEY
    ref_columns: list[str] = field(default_factory=list)
    check: str | None = None        # solo CHECK: la condición como texto SQL


@dataclass
class TableSchema:
    name: str
    columns: list[ColumnSchema]
    constraints: list[ConstraintSchema] = field(default_factory=list)
    next_rowid: int = 1             # siguiente _rowid a asignar (ver dbms.storage)

    def column(self, name: str) -> ColumnSchema | None:
        return next((c for c in self.columns if c.name == name), None)

    @property
    def column_names(self) -> list[str]:
        return [c.name for c in self.columns]

    @property
    def primary_key(self) -> ConstraintSchema | None:
        return next((c for c in self.constraints if c.kind == PRIMARY_KEY), None)

    def constraints_of(self, kind: str) -> list[ConstraintSchema]:
        return [c for c in self.constraints if c.kind == kind]


@dataclass
class IndexSchema:
    name: str
    table: str
    columns: list[str]
    unique: bool = False
    implicit: bool = False          # creado por una PK/UNIQUE, no por CREATE INDEX


@dataclass
class Catalog:
    tables: dict[str, TableSchema] = field(default_factory=dict)
    indexes: dict[str, IndexSchema] = field(default_factory=dict)

    def table(self, name: str) -> TableSchema | None:
        return self.tables.get(name)

    def add_table(self, table: TableSchema) -> None:
        if table.name in self.tables:
            raise KeyError(f"la tabla '{table.name}' ya existe")
        self.tables[table.name] = table

    def drop_table(self, name: str) -> list[IndexSchema]:
        """Quita la tabla y sus índices; devuelve los índices quitados para
        que el almacenamiento borre sus archivos."""
        del self.tables[name]
        dropped = self.indexes_on(name)
        for idx in dropped:
            del self.indexes[idx.name]
        return dropped

    def index(self, name: str) -> IndexSchema | None:
        return self.indexes.get(name)

    def add_index(self, index: IndexSchema) -> None:
        if index.name in self.indexes:
            raise KeyError(f"el índice '{index.name}' ya existe")
        self.indexes[index.name] = index

    def drop_index(self, name: str) -> IndexSchema:
        return self.indexes.pop(name)

    def indexes_on(self, table: str) -> list[IndexSchema]:
        return [i for i in self.indexes.values() if i.table == table]

    def index_for(self, table: str, column: str) -> IndexSchema | None:
        """Un índice de una sola columna sobre `table.column`, si lo hay: el
        que puede usar un WHERE `column = valor`."""
        return next((i for i in self.indexes.values()
                     if i.table == table and i.columns == [column]), None)


# ───────────────────────────── CREATE TABLE -> TableSchema ─────────────────────────────

def table_from_ast(stmt: ast.CreateTable, catalog: Catalog | None = None) -> TableSchema:
    """Convierte un CREATE TABLE en su esquema. No valida (columnas
    repetidas, tablas referenciadas inexistentes, ...): eso es trabajo de la
    semántica, que corre antes. `catalog` solo se usa para completar una
    FOREIGN KEY sin columnas referenciadas con la PK de la otra tabla."""
    columns = [ColumnSchema(c.name, c.type, c.not_null) for c in stmt.columns]
    table = TableSchema(stmt.name, columns)
    used: set[str] = set()

    def name_for(given: str | None, generated: str) -> str:
        name = given or generated
        if given is None:
            n = 2
            while name in used:
                name = f"{generated}_{n}"
                n += 1
        used.add(name)
        return name

    checks = 0
    for c in stmt.constraints:
        if isinstance(c, ast.PrimaryKey):
            table.constraints.append(ConstraintSchema(PRIMARY_KEY, name_for(c.name, f"pk_{stmt.name}"), list(c.columns)))
            for col in c.columns:
                if (schema := table.column(col)) is not None:
                    schema.not_null = True
        elif isinstance(c, ast.Unique):
            table.constraints.append(ConstraintSchema(UNIQUE, name_for(c.name, f"uq_{stmt.name}_{'_'.join(c.columns)}"),
                                                      list(c.columns)))
        elif isinstance(c, ast.ForeignKey):
            table.constraints.append(ConstraintSchema(FOREIGN_KEY, name_for(c.name, f"fk_{stmt.name}_{'_'.join(c.columns)}"),
                                                      list(c.columns), c.ref_table, list(c.ref_columns)))
        elif isinstance(c, ast.Check):
            checks += 1
            # Una condición escrita en varias líneas se guarda en una (metadata.txt es por líneas).
            condition = re.sub(r"\s*[\r\n]+\s*", " ", c.text)
            table.constraints.append(ConstraintSchema(CHECK, name_for(c.name, f"ck_{stmt.name}_{checks}"), check=condition))
    # Al final: una FK a la propia tabla puede aparecer antes que su PK.
    for fk in table.constraints_of(FOREIGN_KEY):
        if not fk.ref_columns:
            fk.ref_columns = _primary_key_of(fk.ref_table, table, catalog)
    return table


def _primary_key_of(ref: str, table: TableSchema, catalog: Catalog | None) -> list[str]:
    """Columnas de la PK de `ref`; la tabla en construcción puede
    referenciarse a sí misma. Lista vacía si no se puede saber (la semántica
    reporta la tabla inexistente)."""
    target = table if ref == table.name else (catalog.table(ref) if catalog else None)
    pk = target.primary_key if target else None
    return list(pk.columns) if pk else []
