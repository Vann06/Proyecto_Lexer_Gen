"""Ejecución de subconsultas (D4): el `subquery_runner` del evaluador.

`eval.py` ya resuelve IN / NOT IN, EXISTS / NOT EXISTS y la cardinalidad de
una escalar (EXE003, EXE006); le falta quien corra el SELECT interno. Eso es
`runner(db)`: un callback `(select, env) -> Result` que `executor.execute`
instala en el entorno de cada sentencia.

Correlación: la subconsulta se ejecuta con el entorno de la fila externa, y
como `select.select` apila sus tablas encima, `Environment.resolve` encuentra
primero las internas y después las externas. No hace falta pasar nada más.

Caché: una subconsulta que no nombra columnas de afuera da siempre lo mismo
dentro de una sentencia, así que se ejecuta una sola vez. Las correlacionadas
se ejecutan por fila externa. El runner (y su caché) es nuevo en cada
sentencia: nunca sirve datos de antes de un INSERT/UPDATE/DELETE.
"""
from dataclasses import fields, is_dataclass

from dbms import ast
from dbms.select import select


def runner(db):
    catalog = None
    cache, correlated = {}, {}

    def run(sel, env):
        nonlocal catalog
        key = id(sel)
        if key in cache:
            return cache[key]
        if key not in correlated:
            catalog = catalog or db.load_catalog()
            correlated[key] = is_correlated(sel, catalog)
        result = select(sel, db, env)
        if not correlated[key]:
            cache[key] = result
        return result

    run.cache = cache          # visible para las pruebas
    return run


def is_correlated(sel: ast.Select, catalog) -> bool:
    """¿La subconsulta (o alguna anidada) nombra una columna que no es de sus
    propias tablas? Ante la duda (una tabla que no está en el catálogo) se
    responde que sí: no cachear nunca da un resultado incorrecto."""
    return _references_outside(sel, catalog, [])


def _references_outside(sel, catalog, visible) -> bool:
    scope = {}
    for ref in ([sel.from_table] if sel.from_table else []) + [j.table for j in sel.joins]:
        table = catalog.table(ref.name)
        if table is None:
            return True
        scope[ref.ref_name] = set(table.column_names)
    visible = [scope, *visible]
    aliases = {i.alias for i in sel.items if isinstance(i, ast.ExprItem) and i.alias}

    def resolves(col: ast.ColumnRef) -> bool:
        if col.table is not None:
            return any(col.table in s for s in visible)
        return col.column in aliases or any(col.column in cols for s in visible for cols in s.values())

    def walk(node) -> bool:
        if isinstance(node, list):
            return any(walk(n) for n in node)
        if isinstance(node, ast.ColumnRef):
            return not resolves(node)
        if isinstance(node, ast.Select):
            return _references_outside(node, catalog, visible)
        if is_dataclass(node) and not isinstance(node, (ast.Pos, ast.DataType)):
            return any(walk(getattr(node, f.name)) for f in fields(node))
        return False

    exprs = [i.expr for i in sel.items if isinstance(i, ast.ExprItem)]
    exprs += [j.on for j in sel.joins if j.on is not None]
    exprs += [sel.where, *sel.group_by, sel.having, *(o.expr for o in sel.order_by)]
    return walk([e for e in exprs if e is not None])
