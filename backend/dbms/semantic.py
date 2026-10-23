"""Análisis semántico: el AST contra el catálogo, antes de ejecutar.

Revisa que lo que la sentencia nombra exista y que los tipos encajen, y
acumula `Problem("err", "SEMxxx", ...)` con línea y columna sin detenerse en
el primero. El ejecutor (fase D) confía en esto: no vuelve a revisar tablas,
columnas ni tipos.

Dos entradas:
- `check(stmt, catalog)`: una sentencia contra un catálogo (la usa la
  ejecución, sentencia por sentencia).
- `check_script(stmts, catalog, data)`: un script entero sin ejecutar nada
  (botón ANALIZAR). Trabaja sobre una copia del catálogo y le aplica el
  efecto de cada sentencia correcta (CREATE/DROP TABLE, INDEX, DATABASE,
  USE), así un INSERT ve la tabla creada dos líneas antes.

Nombres: cada SELECT abre un ámbito con las tablas de su FROM/JOIN, colgado
del ámbito de la consulta que lo contiene. Una columna sin calificar se busca
primero en el ámbito propio y después hacia afuera: así una subconsulta
correlacionada ve las tablas de la consulta externa.

Tipos desconocidos (`None`): si algo ya falló (tabla inexistente, por
ejemplo), su tipo es desconocido y las reglas que dependen de él se callan,
para no reportar errores en cascada.

Códigos (tabla completa en PLAN.md):
    SEM001 tabla inexistente          SEM010 índice inexistente
    SEM002 tabla ya existe            SEM011 índice ya existe
    SEM003 columna inexistente        SEM012 subconsulta con != 1 columna
    SEM004 referencia ambigua         SEM013 agregado mal usado / GROUP BY
    SEM005 calificador inválido       SEM014 función desconocida o aridad
    SEM006 tipos incompatibles        SEM015 base de datos inexistente
    SEM007 INSERT: cantidad valores   SEM016 base de datos ya existe
    SEM008 valor inválido p/columna   SEM017 nombre repetido en una lista
    SEM009 NULL en columna NOT NULL   SEM018 definición de tabla inválida
    SEM099 no soportado todavía
"""

from __future__ import annotations

import copy
from dataclasses import fields, is_dataclass

from dbms import ast, sqltypes
from dbms.catalog import Catalog, IndexSchema, TableSchema, table_from_ast
from dbms.frontend.parse import Problem
from dbms.storage import DEFAULT_DATABASE, DataDir

AGGREGATES = ("COUNT", "SUM", "AVG", "MIN", "MAX")

# Tipo de un NULL literal: compatible con todo. Distinto de None (desconocido).
NULL = ast.DataType("NULL")
BOOLEAN = sqltypes.BOOLEAN

_FAMILY_NAME = {"numeric": "número", "text": "texto", "bool": "booleano", "date": "fecha"}


def _tname(t: ast.DataType) -> str:
    return "NULL" if t is NULL else _FAMILY_NAME[sqltypes.family(t)]


def _is(t: ast.DataType | None, family: str) -> bool:
    """¿`t` sirve donde se espera `family`? Desconocido y NULL sirven siempre."""
    return t is None or t is NULL or sqltypes.family(t) == family


def _sql(e) -> str:
    """Una expresión del AST como texto, para los mensajes."""
    if isinstance(e, ast.Literal):
        if e.kind == "string":
            return "'" + str(e.value).replace("'", "''") + "'"
        if e.kind == "bool":
            return "TRUE" if e.value else "FALSE"
        return "NULL" if e.kind == "null" else str(e.value)
    if isinstance(e, ast.ColumnRef):
        return str(e)
    if isinstance(e, ast.Binary):
        return f"{_sql(e.left)} {e.op} {_sql(e.right)}"
    if isinstance(e, ast.Unary):
        return f"NOT {_sql(e.operand)}" if e.op == "NOT" else f"{e.op}{_sql(e.operand)}"
    if isinstance(e, ast.FuncCall):
        args = "*" if e.star else ", ".join(_sql(a) for a in e.args)
        return f"{e.name}({'DISTINCT ' if e.distinct else ''}{args})"
    if isinstance(e, (ast.ScalarSubquery, ast.InSubquery, ast.Exists)):
        return "(SELECT …)"
    return "la expresión"


def _has_aggregate(e) -> bool:
    """¿Hay un agregado en `e` (sin entrar a subconsultas, que agrupan aparte)?"""
    if isinstance(e, ast.FuncCall) and e.name in AGGREGATES:
        return True
    if isinstance(e, (ast.ScalarSubquery, ast.Exists)):
        return False
    if isinstance(e, ast.InSubquery):
        return _has_aggregate(e.operand)
    return any(_has_aggregate(c) for c in _children(e))


def _children(e) -> list:
    """Sub-expresiones directas de un nodo de expresión."""
    out = []
    for f in fields(e):
        value = getattr(e, f.name)
        for v in (value if isinstance(value, list) else [value]):
            if is_dataclass(v) and not isinstance(v, (ast.Pos, ast.Select, ast.DataType)):
                out.append(v)
    return out


class Scope:
    """Las tablas visibles en un SELECT (o UPDATE/DELETE): pares
    (nombre con que se las cita, esquema). Esquema None = la tabla no existe
    (ya reportado); sus columnas se aceptan en silencio."""

    def __init__(self, sources: list[tuple[str, TableSchema | None]] | None = None,
                 parent: Scope | None = None):
        self.sources = sources or []
        self.parent = parent

    def source(self, name: str):
        return next((s for s in self.sources if s[0] == name), None)


class Analyzer:
    def __init__(self, catalog: Catalog, data: DataDir | None = None, database: str = DEFAULT_DATABASE):
        self.catalog = catalog
        self.data = data
        self.database = database
        self.problems: list[Problem] = []
        # Bases conocidas y catálogos ya cargados (para USE dentro de un script).
        self._databases = set(data.databases()) | {database} if data else {database}
        self._catalogs = {database: catalog}
        self._fallback = ast.Pos(1, 1)

    # ───────────── Reporte ─────────────

    def _err(self, code: str, msg: str, pos: ast.Pos | None = None, level: str = "err") -> None:
        pos = pos or self._fallback
        self.problems.append(Problem(level, code, msg, pos.line, pos.col))

    # ───────────── Entradas ─────────────

    def check(self, stmt: ast.Statement) -> list[Problem]:
        start = len(self.problems)
        self._fallback = stmt.pos
        method = getattr(self, "_stmt_" + type(stmt.body).__name__)
        method(stmt.body)
        return self.problems[start:]

    def check_script(self, statements: list[ast.Statement]) -> list[Problem]:
        for stmt in statements:
            if not any(p.level == "err" for p in self.check(stmt)):
                self._apply(stmt.body)
        return self.problems

    def _apply(self, s) -> None:
        """Efecto de una sentencia correcta sobre el catálogo simulado."""
        c = self.catalog
        if isinstance(s, ast.CreateTable) and c.table(s.name) is None:
            c.add_table(table_from_ast(s, c))
        elif isinstance(s, ast.DropTable) and c.table(s.name) is not None:
            c.drop_table(s.name)
        elif isinstance(s, ast.CreateIndex) and c.index(s.name) is None:
            c.add_index(IndexSchema(s.name, s.table, list(s.columns), unique=s.unique))
        elif isinstance(s, ast.DropIndex) and c.index(s.name) is not None:
            c.drop_index(s.name)
        elif isinstance(s, ast.CreateDatabase) and s.name not in self._databases:
            self._databases.add(s.name)
            self._catalogs[s.name] = Catalog()
        elif isinstance(s, ast.DropDatabase):
            self._databases.discard(s.name)
            self._catalogs.pop(s.name, None)
        elif isinstance(s, ast.UseDatabase):
            if s.name not in self._catalogs:
                self._catalogs[s.name] = self.data.open(s.name).load_catalog() if self.data else Catalog()
            self.database, self.catalog = s.name, self._catalogs[s.name]

    # ───────────── Bases de datos ─────────────

    def _stmt_CreateDatabase(self, s: ast.CreateDatabase):
        if s.name in self._databases and not s.if_not_exists:
            self._err("SEM016", f"la base de datos '{s.name}' ya existe")

    def _stmt_DropDatabase(self, s: ast.DropDatabase):
        if s.name not in self._databases and not s.if_exists:
            self._err("SEM015", f"la base de datos '{s.name}' no existe")

    def _stmt_UseDatabase(self, s: ast.UseDatabase):
        if s.name not in self._databases:
            self._err("SEM015", f"la base de datos '{s.name}' no existe")

    def _stmt_AlterDatabase(self, s: ast.AlterDatabase):
        self._err("SEM099", "ALTER DATABASE todavía no se soporta")

    def _stmt_ShowDatabases(self, s):
        pass

    def _stmt_ShowTables(self, s):
        pass

    def _stmt_ShowColumns(self, s: ast.ShowColumns):
        self._table(s.table)

    # ───────────── Tablas ─────────────

    def _table(self, name: str, pos: ast.Pos | None = None) -> TableSchema | None:
        t = self.catalog.table(name)
        if t is None:
            self._err("SEM001", f"la tabla '{name}' no existe", pos)
        return t

    def _stmt_CreateTable(self, s: ast.CreateTable):
        if self.catalog.table(s.name) is not None:
            if not s.if_not_exists:
                self._err("SEM002", f"la tabla '{s.name}' ya existe")
            return
        seen: set[str] = set()
        for col in s.columns:
            if col.name in seen:
                self._err("SEM017", f"la columna '{col.name}' está repetida en la tabla '{s.name}'", col.pos)
            seen.add(col.name)
            if col.type.length is not None and col.type.length <= 0:
                self._err("SEM018", f"la longitud de {col.type} debe ser mayor que 0", col.pos)
            if col.default is not None:
                self._err("SEM099", f"DEFAULT todavía no se soporta: se ignora en la columna '{col.name}'",
                          col.default.pos, level="warn")
        new = table_from_ast(s, self.catalog)
        pks = [c for c in s.constraints if isinstance(c, ast.PrimaryKey)]
        for extra in pks[1:]:
            self._err("SEM018", f"la tabla '{s.name}' tiene más de una PRIMARY KEY", extra.pos)
        for c in s.constraints:
            if isinstance(c, ast.Check):
                self._condition(c.expr, Scope([(s.name, new)]), "CHECK", aggs=False)
                continue
            self._column_list(new, c.columns, c.pos)
            if isinstance(c, ast.ForeignKey):
                self._foreign_key(s.name, new, c)

    def _column_list(self, table: TableSchema, columns: list[str], pos: ast.Pos | None) -> None:
        """Columnas de una lista (restricción, índice, INSERT): existen y no se repiten."""
        seen: set[str] = set()
        for name in columns:
            if table.column(name) is None:
                self._err("SEM003", f"la tabla '{table.name}' no tiene la columna '{name}'", pos)
            elif name in seen:
                self._err("SEM017", f"la columna '{name}' está repetida en la lista", pos)
            seen.add(name)

    def _foreign_key(self, name: str, new: TableSchema, fk: ast.ForeignKey) -> None:
        ref = new if fk.ref_table == name else self.catalog.table(fk.ref_table)
        if ref is None:
            self._err("SEM001", f"la tabla referenciada '{fk.ref_table}' no existe", fk.pos)
            return
        ref_cols = list(fk.ref_columns) or (ref.primary_key.columns if ref.primary_key else [])
        if not ref_cols:
            self._err("SEM018", f"la tabla '{ref.name}' no tiene PRIMARY KEY: indique las columnas referenciadas", fk.pos)
            return
        if any(ref.column(c) is None for c in ref_cols):
            self._column_list(ref, ref_cols, fk.pos)
            return
        if len(ref_cols) != len(fk.columns):
            self._err("SEM018", f"la FOREIGN KEY tiene {len(fk.columns)} columna(s) y referencia {len(ref_cols)}", fk.pos)
            return
        keys = [k.columns for k in ref.constraints if k.kind in ("PRIMARY KEY", "UNIQUE")]
        if sorted(ref_cols) not in [sorted(k) for k in keys]:
            self._err("SEM018", f"({', '.join(ref_cols)}) no es PRIMARY KEY ni UNIQUE en '{ref.name}'", fk.pos)
        for mine, theirs in zip(fk.columns, ref_cols):
            a, b = new.column(mine), ref.column(theirs)
            if a is not None and not sqltypes.comparable(a.type, b.type):
                self._err("SEM006", f"'{mine}' es {a.type} y '{ref.name}.{theirs}' es {b.type}: tipos incompatibles", fk.pos)

    def _stmt_DropTable(self, s: ast.DropTable):
        if self.catalog.table(s.name) is None and not s.if_exists:
            self._table(s.name)

    def _stmt_AlterTable(self, s: ast.AlterTable):
        self._err("SEM099", "ALTER TABLE todavía no se soporta")

    # ───────────── Índices ─────────────

    def _stmt_CreateIndex(self, s: ast.CreateIndex):
        if self.catalog.index(s.name) is not None:
            if not s.if_not_exists:
                self._err("SEM011", f"el índice '{s.name}' ya existe")
            return
        table = self._table(s.table)
        if table is not None:
            self._column_list(table, s.columns, None)

    def _stmt_DropIndex(self, s: ast.DropIndex):
        if self.catalog.index(s.name) is None and not s.if_exists:
            self._err("SEM010", f"el índice '{s.name}' no existe")

    # ───────────── DML ─────────────

    def _stmt_Insert(self, s: ast.Insert):
        table = self._table(s.table)
        if table is None:
            return
        if s.columns is not None:
            self._column_list(table, s.columns, None)
            targets = [table.column(c) for c in s.columns]
        else:
            targets = list(table.columns)
        given = {c.name for c in targets if c is not None}
        for col in table.columns:
            if col.not_null and col.name not in given:
                self._err("SEM009", f"la columna '{col.name}' es NOT NULL y el INSERT no le da valor")
        for row in s.rows:
            pos = row[0].pos if row else None
            if len(row) != len(targets):
                self._err("SEM007", f"el INSERT da {len(row)} valor(es) para {len(targets)} columna(s)", pos)
                continue
            for col, value in zip(targets, row):
                if col is not None:
                    self._assign(col, value, Scope(), "VALUES")

    def _stmt_Update(self, s: ast.Update):
        table = self._table(s.table)
        scope = Scope([(s.table, table)])
        seen: set[str] = set()
        for a in s.assignments:
            col = table.column(a.column) if table else None
            if table is not None and col is None:
                self._err("SEM003", f"la tabla '{table.name}' no tiene la columna '{a.column}'", a.pos)
            elif a.column in seen:
                self._err("SEM017", f"la columna '{a.column}' se asigna dos veces", a.pos)
            seen.add(a.column)
            if col is not None:
                self._assign(col, a.value, scope, "SET")
            else:
                self._type(a.value, scope, "SET", aggs=False)
        if s.where is not None:
            self._condition(s.where, scope, "WHERE", aggs=False)

    def _stmt_Delete(self, s: ast.Delete):
        table = self._table(s.table)
        if s.where is not None:
            self._condition(s.where, Scope([(s.table, table)]), "WHERE", aggs=False)

    def _assign(self, col, value, scope: Scope, clause: str) -> None:
        """¿`value` se puede guardar en la columna `col`? Los literales se
        prueban con su valor real (largo de un VARCHAR, fecha válida)."""
        if isinstance(value, ast.Literal):
            if value.value is None:
                if col.not_null:
                    self._err("SEM009", f"la columna '{col.name}' es NOT NULL", value.pos)
                return
            try:
                sqltypes.coerce(value.value, col.type)
            except sqltypes.TypeMismatch as e:
                self._err("SEM008", f"valor inválido para '{col.name}' ({col.type}): {e}", value.pos)
            return
        t = self._type(value, scope, clause, aggs=False)
        if t is not None and t is not NULL and not sqltypes.assignable(col.type, t):
            self._err("SEM008", f"'{col.name}' es {col.type} y {_sql(value)} es {_tname(t)}", value.pos)

    # ───────────── SELECT ─────────────

    def _stmt_Select(self, s: ast.Select):
        self._select(s, None)

    def _add_source(self, scope: Scope, ref: ast.TableRef) -> None:
        table = self._table(ref.name, ref.pos)
        if scope.source(ref.ref_name) is not None:
            self._err("SEM004", f"'{ref.ref_name}' aparece dos veces en el FROM: use alias distintos", ref.pos)
            return
        scope.sources.append((ref.ref_name, table))

    def _select(self, s: ast.Select, parent: Scope | None) -> list | None:
        """Revisa un SELECT y devuelve el tipo de cada columna del resultado
        (None si no se puede saber cuántas son, por una tabla inexistente)."""
        scope = Scope([], parent)
        if s.from_table is not None:
            self._add_source(scope, s.from_table)
        for j in s.joins:
            if j.kind != "INNER":
                self._err("SEM099", f"{j.kind} JOIN todavía no se soporta (use INNER JOIN)", j.pos)
            self._add_source(scope, j.table)
            if j.on is not None:
                self._condition(j.on, scope, "ON", aggs=False)
        if s.where is not None:
            self._condition(s.where, scope, "WHERE", aggs=False)
        for g in s.group_by:
            self._type(g, scope, "GROUP BY", aggs=False)

        types: list | None = []
        aliases: dict[str, ast.DataType | None] = {}
        for item in s.items:
            if isinstance(item, ast.Star):
                expanded = self._star(item, scope)
                types = None if expanded is None or types is None else types + expanded
            else:
                t = self._type(item.expr, scope, "SELECT", aggs=True)
                if types is not None:
                    types.append(t)
                if item.alias:
                    aliases[item.alias] = t
        if s.having is not None:
            self._condition(s.having, scope, "HAVING", aggs=True)
        for o in s.order_by:
            self._type(o.expr, scope, "ORDER BY", aggs=True, aliases=aliases)

        exprs = [i.expr for i in s.items if isinstance(i, ast.ExprItem)] + [o.expr for o in s.order_by]
        if s.group_by or s.having is not None or any(_has_aggregate(e) for e in exprs):
            self._grouping(s, scope, aliases)
        return types

    def _star(self, item: ast.Star, scope: Scope) -> list | None:
        if item.table is not None:
            src = scope.source(item.table)
            if src is None:
                self._err("SEM005", f"'{item.table}' no es una tabla ni un alias del FROM", item.pos)
                return None
            sources = [src]
        else:
            sources = scope.sources
            if not sources:
                self._err("SEM003", "SELECT * necesita un FROM", item.pos)
                return None
        if any(schema is None for _, schema in sources):
            return None
        return [c.type for _, schema in sources for c in schema.columns]

    # ───────────── GROUP BY ─────────────

    def _grouping(self, s: ast.Select, scope: Scope, aliases: dict) -> None:
        """En una consulta agrupada, toda columna fuera de un agregado debe
        estar en el GROUP BY (o ser de una consulta externa)."""
        keys = {self._key(g, scope) for g in s.group_by}

        def walk(e, clause: str, allow_alias: bool) -> None:
            if self._key(e, scope) in keys:
                return
            if isinstance(e, ast.FuncCall) and e.name in AGGREGATES:
                return
            if isinstance(e, (ast.ScalarSubquery, ast.Exists)):
                return
            if isinstance(e, ast.InSubquery):
                walk(e.operand, clause, allow_alias)
                return
            if isinstance(e, ast.ColumnRef):
                if allow_alias and e.table is None and e.column in aliases:
                    return
                ident = self._resolve(e, scope, report=False)
                if ident is not None and ident[1][0] == 0:
                    self._err("SEM013", f"la columna '{e}' debe estar en GROUP BY o dentro de un agregado ({clause})", e.pos)
                return
            for child in _children(e):
                walk(child, clause, allow_alias)

        for item in s.items:
            if isinstance(item, ast.Star):
                self._err("SEM013", "SELECT * no se puede usar con GROUP BY ni con agregados", item.pos)
            else:
                walk(item.expr, "SELECT", False)
        if s.having is not None:
            walk(s.having, "HAVING", False)
        for o in s.order_by:
            walk(o.expr, "ORDER BY", True)

    def _key(self, e, scope: Scope):
        """Forma de una expresión sin posiciones, con las columnas resueltas:
        `nombre` y `e.nombre` dan la misma clave si son la misma columna."""
        if isinstance(e, list):
            return tuple(self._key(v, scope) for v in e)
        if isinstance(e, ast.ColumnRef):
            ident = self._resolve(e, scope, report=False)
            return ("col", ident[1] if ident else str(e))
        if isinstance(e, ast.Select):
            return ("select", id(e))        # dos subconsultas nunca son "la misma expresión"
        if is_dataclass(e):
            return (type(e).__name__,) + tuple(self._key(getattr(e, f.name), scope)
                                               for f in fields(e) if f.name != "pos")
        return e

    # ───────────── Nombres ─────────────

    def _resolve(self, ref: ast.ColumnRef, scope: Scope, report: bool = True):
        """Busca la columna. Devuelve (tipo, (profundidad, tabla, columna)), o
        None si no se encontró o no se puede saber. Profundidad 0 = el SELECT
        actual; 1 = el que lo contiene; ..."""
        def err(code, msg):
            if report:
                self._err(code, msg, ref.pos)

        depth, s = 0, scope
        if ref.table is not None:
            while s is not None:
                src = s.source(ref.table)
                if src is not None:
                    name, schema = src
                    if schema is None:
                        return None
                    col = schema.column(ref.column)
                    if col is None:
                        err("SEM003", f"la tabla '{schema.name}' no tiene la columna '{ref.column}'")
                        return None
                    return col.type, (depth, name, col.name)
                s, depth = s.parent, depth + 1
            err("SEM005", f"'{ref.table}' no es una tabla ni un alias del FROM")
            return None
        while s is not None:
            hits = [(name, schema.column(ref.column)) for name, schema in s.sources
                    if schema is not None and schema.column(ref.column) is not None]
            if len(hits) > 1:
                names = ", ".join(n for n, _ in hits)
                err("SEM004", f"la columna '{ref.column}' es ambigua: existe en {names} (use {hits[0][0]}.{ref.column})")
                return None
            if hits:
                name, col = hits[0]
                return col.type, (depth, name, col.name)
            if any(schema is None for _, schema in s.sources):
                return None
            s, depth = s.parent, depth + 1
        tables = [n for n, _ in scope.sources]
        where = f" en {', '.join(tables)}" if tables else " (la consulta no tiene FROM)"
        err("SEM003", f"la columna '{ref.column}' no existe{where}")
        return None

    # ───────────── Expresiones ─────────────

    def _condition(self, e, scope: Scope, clause: str, aggs: bool) -> None:
        t = self._type(e, scope, clause, aggs)
        if not _is(t, "bool"):
            self._err("SEM006", f"la condición de {clause} debe ser booleana; {_sql(e)} es {_tname(t)}", e.pos)

    def _compare(self, a, ta, b, tb, pos) -> None:
        """¿Se pueden comparar `a` y `b`? Una cadena literal se compara con
        una fecha si tiene formato de fecha válido."""
        if ta is None or tb is None or ta is NULL or tb is NULL or sqltypes.comparable(ta, tb):
            return
        for lit, other in ((a, tb), (b, ta)):
            if isinstance(lit, ast.Literal) and lit.kind == "string" and other.name == "DATE":
                try:
                    sqltypes.coerce(lit.value, sqltypes.DATE)
                except sqltypes.TypeMismatch as e:
                    self._err("SEM006", str(e), lit.pos)
                return
        self._err("SEM006", f"no se puede comparar {_sql(a)} ({_tname(ta)}) con {_sql(b)} ({_tname(tb)})", pos)

    def _expect(self, e, t, family: str, what: str) -> None:
        if not _is(t, family):
            self._err("SEM006", f"{what} espera {_FAMILY_NAME[family]} y {_sql(e)} es {_tname(t)}", e.pos)

    def _subquery(self, select: ast.Select, scope: Scope, pos) -> ast.DataType | None:
        """Subconsulta que debe dar exactamente una columna; devuelve su tipo."""
        types = self._select(select, scope)
        if types is None:
            return None
        if len(types) != 1:
            self._err("SEM012", f"la subconsulta debe devolver una sola columna y devuelve {len(types)}", pos)
            return None
        return types[0]

    def _type(self, e, scope: Scope, clause: str, aggs: bool, in_agg: bool = False,
              aliases: dict | None = None) -> ast.DataType | None:
        """Tipo de la expresión `e`, reportando lo que esté mal adentro.
        `aggs`: si se permiten agregados en esta cláusula."""
        def sub(x, **kw):
            return self._type(x, scope, clause, aggs, kw.get("in_agg", in_agg), aliases)

        if isinstance(e, ast.Literal):
            return {"int": sqltypes.INT, "float": sqltypes.FLOAT, "bool": BOOLEAN, "null": NULL,
                    "string": ast.DataType("VARCHAR", max(1, len(str(e.value))))}[e.kind]

        if isinstance(e, ast.ColumnRef):
            if aliases is not None and e.table is None and e.column in aliases:
                return aliases[e.column]
            found = self._resolve(e, scope)
            return found[0] if found else None

        if isinstance(e, ast.Unary):
            t = sub(e.operand)
            if e.op == "NOT":
                self._expect(e.operand, t, "bool", "NOT")
                return BOOLEAN
            self._expect(e.operand, t, "numeric", f"el operador {e.op}")
            return t

        if isinstance(e, ast.Binary):
            lt, rt = sub(e.left), sub(e.right)
            if e.op in ("AND", "OR"):
                self._expect(e.left, lt, "bool", e.op)
                self._expect(e.right, rt, "bool", e.op)
                return BOOLEAN
            if e.op in ("=", "<>", "<", "<=", ">", ">="):
                self._compare(e.left, lt, e.right, rt, e.pos)
                return BOOLEAN
            if e.op == "||":
                self._expect(e.left, lt, "text", "||")
                self._expect(e.right, rt, "text", "||")
                return ast.DataType("VARCHAR", None)
            self._expect(e.left, lt, "numeric", f"el operador {e.op}")
            self._expect(e.right, rt, "numeric", f"el operador {e.op}")
            if lt is None or rt is None:
                return None
            if lt is NULL or rt is NULL:
                return NULL
            if not (_is(lt, "numeric") and _is(rt, "numeric")):
                return None
            return sqltypes.FLOAT if sqltypes.FLOAT in (lt, rt) else sqltypes.INT

        if isinstance(e, ast.IsNull):
            sub(e.operand)
            return BOOLEAN

        if isinstance(e, ast.InList):
            t = sub(e.operand)
            for item in e.items:
                self._compare(e.operand, t, item, sub(item), item.pos)
            return BOOLEAN

        if isinstance(e, ast.Like):
            self._expect(e.operand, sub(e.operand), "text", "LIKE")
            self._expect(e.pattern, sub(e.pattern), "text", "LIKE")
            return BOOLEAN

        if isinstance(e, ast.Between):
            t = sub(e.operand)
            self._compare(e.operand, t, e.low, sub(e.low), e.low.pos)
            self._compare(e.operand, t, e.high, sub(e.high), e.high.pos)
            return BOOLEAN

        if isinstance(e, ast.FuncCall):
            return self._function(e, scope, clause, aggs, in_agg, aliases)

        if isinstance(e, ast.ScalarSubquery):
            return self._subquery(e.select, scope, e.pos)

        if isinstance(e, ast.InSubquery):
            t = sub(e.operand)
            inner = self._subquery(e.select, scope, e.pos)
            first = e.select.items[0]
            self._compare(e.operand, t, first.expr if isinstance(first, ast.ExprItem) else e, inner, e.pos)
            return BOOLEAN

        if isinstance(e, ast.Exists):
            self._select(e.select, scope)
            return BOOLEAN

        return None

    def _function(self, e: ast.FuncCall, scope, clause, aggs, in_agg, aliases):
        if e.name not in AGGREGATES:
            self._err("SEM014", f"función desconocida '{e.name}'; las disponibles son {', '.join(AGGREGATES)}", e.pos)
            for a in e.args:
                self._type(a, scope, clause, aggs, in_agg, aliases)
            return None
        if not aggs:
            self._err("SEM013", f"no se permite {e.name}(…) en {clause}", e.pos)
        elif in_agg:
            self._err("SEM013", f"no se puede usar {e.name}(…) dentro de otro agregado", e.pos)
        if e.star and e.name != "COUNT":
            self._err("SEM014", f"{e.name}(*) no existe; solo COUNT(*)", e.pos)
            return None
        if not e.star and len(e.args) != 1:
            self._err("SEM014", f"{e.name} recibe un argumento y recibió {len(e.args)}", e.pos)
            return None
        if e.name == "COUNT":
            if e.args:
                self._type(e.args[0], scope, clause, aggs, True, aliases)
            return sqltypes.INT
        t = self._type(e.args[0], scope, clause, aggs, True, aliases)
        if e.name in ("SUM", "AVG"):
            self._expect(e.args[0], t, "numeric", e.name)
            if e.name == "AVG":
                return sqltypes.FLOAT
        return None if t is NULL else t


# ───────────────────────────── API ─────────────────────────────

def check(stmt: ast.Statement, catalog: Catalog) -> list[Problem]:
    """Problemas semánticos de una sentencia contra `catalog` (no lo modifica)."""
    return Analyzer(catalog).check(stmt)


def check_script(statements: list[ast.Statement], catalog: Catalog, data: DataDir | None = None,
                 database: str = DEFAULT_DATABASE) -> list[Problem]:
    """Problemas de un script completo sin ejecutarlo. `catalog` no se
    modifica: se trabaja sobre una copia. `data` permite que USE abra otras
    bases existentes."""
    return Analyzer(copy.deepcopy(catalog), data, database).check_script(statements)
