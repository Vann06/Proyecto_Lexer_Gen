"""INNER JOIN (D3): extensión `join.rows` que llama `select.select`.

Cada entorno que llega (una fila de las tablas anteriores) se combina con
las filas de la tabla unida, y queda si ON da TRUE. Las tablas unidas
comparten el ámbito local, así `e.id` y `i.nota` se resuelven igual que si
vinieran de una sola tabla; los ámbitos externos (subconsultas
correlacionadas) quedan debajo, sin tocar.

Dos estrategias por JOIN (`Joined.plan` dice cuál se usó en cada uno):
- **índice**: si ON tiene una igualdad `unida.col = <expresión de las tablas
  anteriores>` y `unida.col` tiene un índice de una columna (incluidos los
  implícitos de PK/UNIQUE), por cada fila de la izquierda se busca la clave
  en el índice y solo se leen esas filas del CSV (por posición). El índice y
  las filas se cargan una vez por JOIN, no por fila.
- **recorrido completo**: bucles anidados sobre toda la tabla unida.
En los dos casos se evalúa después el ON completo, así las condiciones
extra (`AND c.creditos >= 4`) se respetan y el resultado es el mismo.

Solo INNER: la semántica rechaza LEFT/RIGHT/CROSS (SEM099) antes de llegar.
"""
from dataclasses import fields, is_dataclass

from dbms import ast, index
from dbms.eval import Environment, evaluate


class Joined(list):
    """Los entornos combinados, más la estrategia usada en cada JOIN."""
    plan: list[str]


def combine(env: Environment, name: str, values: dict) -> Environment:
    """`env` con la fila `values` agregada al ámbito local como `name`."""
    local = {**env.scopes[0], name: values} if env.scopes else {name: values}
    return Environment([local, *env.scopes[1:]], env.subquery_runner, env.aggregate_runner)


def rows(stmt, db, envs):
    catalog = db.load_catalog()
    plan = []
    for join in stmt.joins:
        table = catalog.table(join.table.name)
        name = join.table.ref_name
        found = _indexable(join.on, name, table, catalog)
        if found is None:
            plan.append(f"JOIN {name}: recorrido completo")
            candidates = _all_rows(db, table)
        else:
            idx, other = found
            plan.append(f"JOIN {name}: índice {idx.name}")
            candidates = _by_index(db, table, idx, other)
        joined = []
        for env in envs:
            for row in candidates(env):
                combined = combine(env, name, row.values)
                if join.on is None or evaluate(join.on, combined) is True:
                    joined.append(combined)
        envs = joined
    result = Joined(envs)
    result.plan = plan
    return result


def _all_rows(db, table):
    rows_ = db.read_rows(table)
    return lambda env: rows_


def _by_index(db, table, idx, other):
    """Filas de `table` cuya columna indexada vale `other` en cada entorno.
    Las claves se buscan con un dict armado una sola vez desde el .idx, y las
    filas se leen del CSV por posición, solo las necesarias, con caché."""
    keys = {key: ids for key, ids in index.load_entries(db, table, idx)}
    cache: dict[int, object] = {}

    def candidates(env):
        value = evaluate(other, env)
        ids = keys.get(value, []) if value is not None else []
        missing = [rid for rid in ids if rid not in cache]
        if missing:
            for row in index.read_selected(db, table, missing):
                cache[row.rowid] = row
        return [cache[rid] for rid in ids]

    return candidates


def _indexable(on, name, table, catalog):
    """(índice, expresión) si alguna parte del ON (unidas por AND) es
    `unida.col = expresión` con un índice de una columna sobre `col` y una
    expresión que no depende de la tabla unida. None si no hay."""
    if on is None:
        return None
    for term in index.conjuncts(on):
        if not (isinstance(term, ast.Binary) and term.op == "="):
            continue
        for col, other in ((term.left, term.right), (term.right, term.left)):
            if _is_column_of(col, name, table) and not _depends_on(other, name, table):
                idx = catalog.index_for(table.name, col.column)
                if idx is not None:
                    return idx, other
    return None


def _is_column_of(expr, name, table) -> bool:
    """¿`expr` es exactamente una columna de la tabla unida? Sin calificar
    cuenta si la tabla la tiene: la semántica ya descartó la ambigüedad."""
    return isinstance(expr, ast.ColumnRef) and (
        expr.table == name or (expr.table is None and table.column(expr.column) is not None))


def _depends_on(expr, name, table) -> bool:
    """¿`expr` usa la tabla unida (o una subconsulta, que no se analiza)?
    Si sí, no se puede calcular antes de elegir la fila: no es indexable."""
    if isinstance(expr, (ast.ScalarSubquery, ast.InSubquery, ast.Exists)):
        return True
    if isinstance(expr, ast.ColumnRef):
        return _is_column_of(expr, name, table)
    if is_dataclass(expr) and not isinstance(expr, (ast.Pos, ast.DataType)):
        return any(_depends_on(v, name, table) for f in fields(expr)
                   for v in (getattr(expr, f.name) if isinstance(getattr(expr, f.name), list)
                             else [getattr(expr, f.name)]))
    return False
