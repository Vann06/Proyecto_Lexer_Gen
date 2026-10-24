"""Despacho D1/D2/índices. El llamador debe ejecutar semantic.check antes.

Las mutaciones se calculan y validan completas en memoria antes de escribir.
No ofrece transacciones concurrentes ni recuperación frente a apagados.
"""
from dbms import ast, sqltypes
from dbms.catalog import PRIMARY_KEY, UNIQUE, IndexSchema, table_from_ast
from dbms.eval import Environment, evaluate
from dbms import index
from dbms.result import ExecutionError, Result
from dbms.select import select
from dbms.storage import Row, _atomic_write


def _validate(table, rows, catalog):
    for row in rows:
        for col in table.columns:
            value = sqltypes.coerce(row.values.get(col.name), col.type)
            if value is None and col.not_null:
                raise ExecutionError("EXE002", f"NULL en columna NOT NULL '{table.name}.{col.name}'")
            row.values[col.name] = value
    unique = [(k.name, k.columns) for k in table.constraints if k.kind in (PRIMARY_KEY, UNIQUE)]
    unique += [(i.name, i.columns) for i in catalog.indexes_on(table.name) if i.unique]
    for name, cols in unique:
        seen = set()
        for row in rows:
            key = tuple(row.values[c] for c in cols)
            if None in key:
                continue
            if key in seen:
                raise ExecutionError("EXE001", f"Llave duplicada en '{name}': {key}")
            seen.add(key)


def execute(stmt, db, *, env=None, join_rows=None, group=None):
    """Acepta Statement o su body. Devuelve Result o lanza ExecutionError.

    ExecutionError.to_problem() conserva el formato y ubicación de la API.
    """
    pos = stmt.pos if isinstance(stmt, ast.Statement) else None
    body = stmt.body if isinstance(stmt, ast.Statement) else stmt
    try:
        return _execute(body, db, env or Environment(), join_rows, group)
    except ExecutionError as exc:
        if exc.pos is None:
            exc.pos = pos
        raise
    except sqltypes.TypeMismatch as exc:
        raise ExecutionError("EXE004", str(exc), pos) from exc


def _execute(s, db, env, join_rows, group):
    if isinstance(s, ast.Select):
        return select(s, db, env, join_rows=join_rows, group=group)
    catalog = db.load_catalog()
    if isinstance(s, ast.CreateTable):
        if s.name in catalog.tables and s.if_not_exists:
            return Result(message=f"La tabla '{s.name}' ya existe")
        table = table_from_ast(s, catalog)
        catalog.add_table(table)
        for k in table.constraints:
            if k.kind in (PRIMARY_KEY, UNIQUE):
                name, suffix = k.name, 2
                while name in catalog.indexes:
                    name, suffix = f"{k.name}_{suffix}", suffix + 1
                catalog.add_index(IndexSchema(name, table.name, k.columns, unique=True, implicit=True))
        # Valida nombres de archivo antes de crear nada.
        for idx in catalog.indexes_on(table.name):
            index.index_path(db, idx.name)
        db.create_table_file(table)
        index.rebuild(db, catalog, table, [])
        db.save_catalog(catalog)
        return Result(message=f"Tabla '{s.name}' creada")
    if isinstance(s, ast.DropTable):
        if s.name not in catalog.tables and s.if_exists:
            return Result(message=f"La tabla '{s.name}' no existe")
        table = catalog.table(s.name)
        dropped = catalog.drop_table(s.name)
        db.drop_table_file(s.name)
        for idx in dropped:
            index.index_path(db, idx.name).unlink(missing_ok=True)
        index.positions_path(db, table).unlink(missing_ok=True)
        db.save_catalog(catalog)
        return Result(message=f"Tabla '{s.name}' eliminada")
    if isinstance(s, ast.CreateIndex):
        if s.name in catalog.indexes and s.if_not_exists:
            return Result(message=f"El índice '{s.name}' ya existe")
        idx = IndexSchema(s.name, s.table, s.columns, s.unique)
        catalog.add_index(idx)
        table = catalog.table(s.table)
        rows = db.read_rows(table)
        text = index.build_text(idx, rows)  # UNIQUE antes de modificar archivos
        _atomic_write(index.index_path(db, idx.name), text)
        index.write_positions(db, table)
        db.save_catalog(catalog)
        return Result(message=f"Índice '{s.name}' creado")
    if isinstance(s, ast.DropIndex):
        if s.name not in catalog.indexes and s.if_exists:
            return Result(message=f"El índice '{s.name}' no existe")
        idx = catalog.drop_index(s.name)
        index.index_path(db, idx.name).unlink(missing_ok=True)
        if not catalog.indexes_on(idx.table):
            index.positions_path(db, catalog.table(idx.table)).unlink(missing_ok=True)
        db.save_catalog(catalog)
        return Result(message=f"Índice '{s.name}' eliminado")
    if isinstance(s, (ast.Insert, ast.Update, ast.Delete)):
        table = catalog.table(s.table)
        rows = db.read_rows(table)
        count, plan = 0, None
        if isinstance(s, ast.Insert):
            cols = table.column_names if s.columns is None else s.columns
            for expressions in s.rows:
                if len(expressions) != len(cols):
                    raise ExecutionError("EXE004", "Cantidad de valores distinta a las columnas")
                values = dict.fromkeys(table.column_names)
                values.update({col: evaluate(expr, env) for col, expr in zip(cols, expressions)})
                rows.append(Row(table.next_rowid, values))
                table.next_rowid += 1
                count += 1
        else:
            ids, plan = index.candidates(db, catalog, table, s.where)
            updated = []
            for row in rows:
                row_env = env.push({table.name: row.values})
                matches = (ids is None or row.rowid in ids) and (
                    s.where is None or evaluate(s.where, row_env) is True)
                if matches:
                    count += 1
                    if isinstance(s, ast.Delete):
                        continue
                    # SET es simultáneo: todas las expresiones ven la fila original.
                    values = {a.column: evaluate(a.value, row_env) for a in s.assignments}
                    row = Row(row.rowid, {**row.values, **values})
                updated.append(row)
            rows = updated
        _validate(table, rows, catalog)
        db.write_rows(table, rows)
        index.rebuild(db, catalog, table, rows)
        db.save_catalog(catalog)
        return Result(message=f"{count} filas afectadas", plan=plan)
    if isinstance(s, ast.ShowTables):
        return Result(["tabla"], [[name] for name in catalog.tables])
    if isinstance(s, ast.ShowColumns):
        return Result(["columna", "tipo", "not_null"],
                      [[c.name, str(c.type), c.not_null] for c in catalog.table(s.table).columns])
    raise ExecutionError("EXE099", f"Sentencia aún no soportada: {type(s).__name__}")
