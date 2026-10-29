"""INNER JOIN (D3): extensión `join.rows` que llama `select.select`.

Bucles anidados, un JOIN tras otro: cada entorno que llega (una fila de las
tablas anteriores) se combina con cada fila de la tabla unida, y queda si ON
da TRUE. Las tablas unidas comparten el ámbito local, así `e.id` y `i.nota`
se resuelven igual que si vinieran de una sola tabla; los ámbitos externos
(subconsultas correlacionadas) quedan debajo, sin tocar.

Solo INNER: la semántica rechaza LEFT/RIGHT/CROSS (SEM099) antes de llegar.
"""
from dbms.eval import Environment, evaluate


def combine(env: Environment, name: str, values: dict) -> Environment:
    """`env` con la fila `values` agregada al ámbito local como `name`."""
    local = {**env.scopes[0], name: values} if env.scopes else {name: values}
    return Environment([local, *env.scopes[1:]], env.subquery_runner, env.aggregate_runner)


def rows(stmt, db, envs):
    catalog = db.load_catalog()
    for join in stmt.joins:
        right = db.read_rows(catalog.table(join.table.name))
        name = join.table.ref_name
        joined = []
        for env in envs:
            for row in right:
                combined = combine(env, name, row.values)
                if join.on is None or evaluate(join.on, combined) is True:
                    joined.append(combined)
        envs = joined
    return envs
