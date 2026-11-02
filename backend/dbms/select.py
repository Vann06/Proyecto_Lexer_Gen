"""SELECT D1/D2, con puntos de extensión para JOIN y agrupación (D3)."""
from dataclasses import fields, is_dataclass
from functools import cmp_to_key
from importlib import import_module

from dbms import ast
from dbms.eval import Environment, evaluate
from dbms.index import scan
from dbms.result import ExecutionError, Result


def _extension(module, function):
    try:
        return getattr(import_module(f"dbms.{module}"), function)
    except ModuleNotFoundError as exc:
        if exc.name != f"dbms.{module}":
            raise
        raise ExecutionError("EXE099", f"Falta conectar {module}.{function} (D3)") from exc


def _has_aggregate(node):
    if isinstance(node, ast.FuncCall):
        return True
    if isinstance(node, (ast.ScalarSubquery, ast.InSubquery, ast.Exists)):
        return False
    if isinstance(node, list):
        return any(_has_aggregate(n) for n in node)
    return is_dataclass(node) and any(_has_aggregate(getattr(node, f.name)) for f in fields(node))


def select(stmt, db, env=None, *, join_rows=None, group=None):
    """Extensiones: join_rows(stmt, db, envs), group(stmt, envs, outer) -> iterable[Environment].

    group instala aggregate_runner en cada entorno; los callbacks de subconsulta
    se heredan al agregar ámbitos. La proyección y ORDER BY usan el mismo entorno.
    """
    catalog = db.load_catalog()
    outer = env or Environment()
    plan = "recorrido completo"
    if stmt.from_table:
        ref = stmt.from_table
        table = catalog.table(ref.name)
        # Un filtro sin calificar puede pertenecer a otra tabla en un JOIN.
        rows, plan = scan(db, catalog, table, None if stmt.joins else stmt.where, ref.ref_name)
        envs = [outer.push({ref.ref_name: row.values}) for row in rows]
    else:
        envs = [outer.push({})]
    if stmt.joins:
        if join_rows is None:
            join_rows = _extension("join", "rows")
        joined = join_rows(stmt, db, envs)
        # join.rows puede decir qué estrategia usó cada JOIN (índice o recorrido).
        plan = "; ".join([plan, *getattr(joined, "plan", [])])
        envs = list(joined)
    envs = [e for e in envs if stmt.where is None or evaluate(stmt.where, e) is True]
    if stmt.group_by or stmt.having or _has_aggregate(stmt.items) or _has_aggregate(stmt.order_by):
        if group is None:
            group = _extension("aggregate", "group")
        # `outer`: el grupo vacío de un agregado global no tiene fila propia
        # y necesita las filas externas y el runner de subconsultas.
        envs = list(group(stmt, envs, outer))
    if stmt.having:
        envs = [e for e in envs if evaluate(stmt.having, e) is True]

    projection, columns = [], []
    refs = ([stmt.from_table] if stmt.from_table else []) + [j.table for j in stmt.joins]
    for item in stmt.items:
        if isinstance(item, ast.Star):
            for ref in refs:
                if item.table is None or ref.ref_name == item.table:
                    for col in catalog.table(ref.name).columns:
                        columns.append(col.name)
                        projection.append((ast.ColumnRef(ref.ref_name, col.name, item.pos), None))
        else:
            columns.append(item.alias or (item.expr.column if isinstance(item.expr, ast.ColumnRef) else item.text))
            projection.append((item.expr, item.alias))
    records = []
    seen = set()
    for e in envs:
        values = [evaluate(expr, e) for expr, _ in projection]
        if stmt.distinct:
            key = tuple(values)
            if key in seen:
                continue
            seen.add(key)
        aliases = {alias: value for (_, alias), value in zip(projection, values) if alias}
        order_env = e.push({"": aliases}) if aliases else e
        keys = [evaluate(order.expr, order_env) for order in stmt.order_by]
        records.append((values, keys))

    def compare_records(a, b):
        for order, x, y in zip(stmt.order_by, a[1], b[1]):
            if x is None or y is None:
                difference = (x is None) - (y is None)
            else:
                difference = (x > y) - (x < y)
                if order.descending:
                    difference = -difference
            if difference:
                return difference
        return 0

    if stmt.order_by:
        records.sort(key=cmp_to_key(compare_records))
    start = stmt.offset or 0
    end = None if stmt.limit is None else start + stmt.limit
    return Result(columns, [r[0] for r in records[start:end]], plan=plan)
