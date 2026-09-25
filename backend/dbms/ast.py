"""AST del DBMS: la forma limpia de cada sentencia SQL.

`dbms.builder` lo arma a partir del árbol de ANTLR; de ahí en adelante
(semántica, ejecución) nadie vuelve a tocar clases generadas por ANTLR.

Convenciones:
- Los identificadores sin comillas se normalizan a minúsculas (como hace
  PostgreSQL); los entrecomillados ("Nombre") conservan mayúsculas.
- Las restricciones escritas junto a una columna (`id INT PRIMARY KEY`) se
  guardan igual que las de tabla (`PRIMARY KEY (id)`): una sola forma para
  que la semántica y el catálogo no tengan dos casos.
- Todo nodo lleva `pos` (línea y columna 1-based) para reportar errores.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Union


@dataclass(frozen=True)
class Pos:
    line: int
    col: int


# ───────────────────────────── Expresiones ─────────────────────────────

@dataclass
class Literal:
    value: object              # int | float | str | bool | None
    kind: str                  # "int" | "float" | "string" | "bool" | "null"
    pos: Pos


@dataclass
class ColumnRef:
    table: str | None          # calificador (tabla o alias), si lo hay
    column: str
    pos: Pos

    def __str__(self) -> str:
        return f"{self.table}.{self.column}" if self.table else self.column


@dataclass
class Unary:
    op: str                    # "-" | "+" | "NOT"
    operand: Expr
    pos: Pos


@dataclass
class Binary:
    op: str                    # + - * / % || = <> < <= > >= AND OR
    left: Expr
    right: Expr
    pos: Pos


@dataclass
class IsNull:
    operand: Expr
    negated: bool              # IS NOT NULL
    pos: Pos


@dataclass
class InList:
    operand: Expr
    items: list[Expr]
    negated: bool
    pos: Pos


@dataclass
class Like:
    operand: Expr
    pattern: Expr
    negated: bool
    pos: Pos


@dataclass
class Between:
    operand: Expr
    low: Expr
    high: Expr
    negated: bool
    pos: Pos


@dataclass
class FuncCall:
    name: str                  # en mayúsculas: COUNT, SUM, UPPER, ...
    args: list[Expr]
    star: bool                 # COUNT(*)
    distinct: bool             # COUNT(DISTINCT x)
    pos: Pos


Expr = Union[Literal, ColumnRef, Unary, Binary, IsNull, InList, Like, Between, FuncCall]


# ───────────────────────────── Tipos y restricciones ─────────────────────────────

@dataclass(frozen=True)
class DataType:
    name: str                  # "INT" | "FLOAT" | "CHAR" | "VARCHAR" | "DATE" | "BOOLEAN"
    length: int | None = None  # solo CHAR / VARCHAR

    def __str__(self) -> str:
        return f"{self.name}({self.length})" if self.length is not None else self.name


@dataclass
class PrimaryKey:
    name: str | None
    columns: list[str]
    pos: Pos


@dataclass
class Unique:
    name: str | None
    columns: list[str]
    pos: Pos


@dataclass
class Check:
    name: str | None
    expr: Expr
    text: str                  # la condición tal como se escribió (se guarda en el catálogo)
    pos: Pos


@dataclass
class ForeignKey:
    name: str | None
    columns: list[str]
    ref_table: str
    ref_columns: list[str]     # vacío = la llave primaria de ref_table
    pos: Pos


Constraint = Union[PrimaryKey, Unique, Check, ForeignKey]


@dataclass
class ColumnDef:
    name: str
    type: DataType
    not_null: bool
    default: Expr | None
    pos: Pos


# ───────────────────────────── Sentencias: bases de datos ─────────────────────────────

@dataclass
class CreateDatabase:
    name: str
    if_not_exists: bool


@dataclass
class DropDatabase:
    name: str
    if_exists: bool


@dataclass
class AlterDatabase:
    name: str
    new_name: str


@dataclass
class UseDatabase:
    name: str


@dataclass
class ShowDatabases:
    pass


@dataclass
class ShowTables:
    pass


@dataclass
class ShowColumns:
    table: str


# ───────────────────────────── Sentencias: tablas e índices ─────────────────────────────

@dataclass
class CreateTable:
    name: str
    columns: list[ColumnDef]
    constraints: list[Constraint]
    if_not_exists: bool


@dataclass
class DropTable:
    name: str
    if_exists: bool


@dataclass
class RenameTable:
    new_name: str
    pos: Pos


@dataclass
class RenameColumn:
    column: str
    new_name: str
    pos: Pos


@dataclass
class AddColumn:
    column: ColumnDef
    constraints: list[Constraint]   # las escritas junto a la columna nueva
    pos: Pos


@dataclass
class DropColumn:
    column: str
    pos: Pos


@dataclass
class AddConstraint:
    constraint: Constraint
    pos: Pos


@dataclass
class DropConstraint:
    name: str
    pos: Pos


@dataclass
class AlterColumnType:
    column: str
    type: DataType
    pos: Pos


AlterAction = Union[RenameTable, RenameColumn, AddColumn, DropColumn,
                    AddConstraint, DropConstraint, AlterColumnType]


@dataclass
class AlterTable:
    table: str
    actions: list[AlterAction]


@dataclass
class CreateIndex:
    name: str
    table: str
    columns: list[str]
    unique: bool
    if_not_exists: bool


@dataclass
class DropIndex:
    name: str
    if_exists: bool


# ───────────────────────────── Sentencias: DML ─────────────────────────────

@dataclass
class Insert:
    table: str
    columns: list[str] | None       # None = todas, en el orden de la tabla
    rows: list[list[Expr]]


@dataclass
class Assignment:
    column: str
    value: Expr
    pos: Pos


@dataclass
class Update:
    table: str
    assignments: list[Assignment]
    where: Expr | None


@dataclass
class Delete:
    table: str
    where: Expr | None


# ───────────────────────────── Sentencias: SELECT ─────────────────────────────

@dataclass
class Star:
    table: str | None               # None = `*`; "t" = `t.*`
    pos: Pos


@dataclass
class ExprItem:
    expr: Expr
    alias: str | None
    text: str                       # texto original, para nombrar la columna del resultado
    pos: Pos


SelectItem = Union[Star, ExprItem]


@dataclass
class TableRef:
    name: str
    alias: str | None
    pos: Pos

    @property
    def ref_name(self) -> str:
        """Cómo se la nombra en el resto de la consulta."""
        return self.alias or self.name


@dataclass
class Join:
    kind: str                       # "INNER" | "LEFT" | "RIGHT" | "CROSS"
    table: TableRef
    on: Expr | None                 # None solo en CROSS JOIN
    pos: Pos


@dataclass
class OrderItem:
    expr: Expr
    descending: bool


@dataclass
class Select:
    distinct: bool
    items: list[SelectItem]
    from_table: TableRef | None
    joins: list[Join] = field(default_factory=list)
    where: Expr | None = None
    group_by: list[Expr] = field(default_factory=list)
    having: Expr | None = None
    order_by: list[OrderItem] = field(default_factory=list)
    limit: int | None = None
    offset: int | None = None


StatementBody = Union[
    CreateDatabase, DropDatabase, AlterDatabase, UseDatabase,
    ShowDatabases, ShowTables, ShowColumns,
    CreateTable, DropTable, AlterTable, CreateIndex, DropIndex,
    Insert, Update, Delete, Select,
]


@dataclass
class Statement:
    """Una sentencia del script con su texto y posición de origen."""
    body: StatementBody
    sql: str
    pos: Pos

    @property
    def kind(self) -> str:
        return type(self.body).__name__
