"""Agregados y GROUP BY (D3): extensión `aggregate.group` de `select.select`.

Recibe los entornos que ya pasaron FROM/JOIN/WHERE y devuelve uno por grupo.
Cada uno es el entorno de la primera fila del grupo (las columnas del GROUP
BY valen lo mismo en todas) con un `aggregate_runner` que calcula COUNT, SUM,
AVG, MIN y MAX sobre las filas del grupo. HAVING, ORDER BY y la proyección
los evalúa después `select.py` con ese entorno.

Reglas (las del estándar):
- GROUP BY agrupa por igualdad de valores; todos los NULL forman un grupo.
- Sin GROUP BY hay un solo grupo, aunque no haya filas: `SELECT COUNT(*)`
  de una tabla vacía da 0. Con GROUP BY y sin filas no hay grupos.
- COUNT(*) cuenta filas; COUNT(x) y el resto ignoran NULL. Sin valores,
  COUNT da 0 y los demás NULL. DISTINCT descarta repetidos.
- SUM conserva el tipo (INT o FLOAT); AVG siempre es FLOAT.
"""
from dbms.eval import Environment, evaluate


def group(stmt, envs, outer=None):
    """`outer`: el entorno de la consulta (filas externas y runner de
    subconsultas). Lo usa el grupo vacío de un agregado global, que no tiene
    una fila propia de donde sacarlo."""
    groups: dict[tuple, list] = {}
    if stmt.group_by:
        for env in envs:
            key = tuple(evaluate(g, env) for g in stmt.group_by)
            groups.setdefault(key, []).append(env)
    else:
        groups[()] = list(envs)
    result = []
    for members in groups.values():
        base = members[0] if members else (outer or Environment())
        result.append(Environment(base.scopes, base.subquery_runner, _runner(members)))
    return result


def _runner(members):
    computed = {}

    def run(call, env):
        # La proyección, HAVING y ORDER BY pueden pedir el mismo agregado.
        if id(call) not in computed:
            computed[id(call)] = _compute(call, members)
        return computed[id(call)]

    return run


def _compute(call, members):
    if call.star:
        return len(members)
    values = [v for v in (evaluate(call.args[0], m) for m in members) if v is not None]
    if call.distinct:
        values = list(dict.fromkeys(values))
    if call.name == "COUNT":
        return len(values)
    if not values:
        return None
    if call.name == "SUM":
        return sum(values)
    if call.name == "AVG":
        return sum(values) / len(values)
    return min(values) if call.name == "MIN" else max(values)
