"""Índices de texto ordenados y directorio de posiciones para acceso directo al CSV.

Cada línea .idx es clave JSON|rowid,rowid,...; las claves compuestas son listas.
El archivo <tabla>.csv.pos guarda rowid -> [offset UTF-8, longitud en bytes].
No se confunden saltos de línea dentro de una celda con registros del CSV.
"""
from bisect import bisect_left, bisect_right
import csv
from datetime import date
import io
import json

from dbms import ast, sqltypes
from dbms.eval import evaluate
from dbms.result import ExecutionError
from dbms.storage import Row, StorageError, _atomic_write, _check_filename


def index_path(db, name):
    return db.path / f"{_check_filename(name)}.idx"


def positions_path(db, table):
    return db.table_path(table.name).with_suffix(".csv.pos")


def _sort_key(key):
    return tuple((v is None, v) for v in key)


def build_text(index, rows):
    entries = {}
    for row in rows:
        key = tuple(row.values[c] for c in index.columns)
        ids = entries.setdefault(key, [])
        if index.unique and None not in key and ids:
            raise ExecutionError("EXE001", f"Llave duplicada en '{index.name}': {key}")
        ids.append(row.rowid)
    lines = []
    for key in sorted(entries, key=_sort_key):
        value = key[0] if len(key) == 1 else key
        encoded = json.dumps(value, ensure_ascii=True,
                             default=lambda v: v.isoformat() if isinstance(v, date) else str(v))
        # El separador nunca aparece literalmente dentro de una clave.
        encoded = encoded.replace("|", "\\u007c")
        lines.append(encoded + "|" + ",".join(map(str, entries[key])))
    return "\n".join(lines) + ("\n" if lines else "")


def write_positions(db, table):
    # csv.reader consume tantas líneas físicas como requiera cada registro.
    offsets = {}
    with open(db.table_path(table.name), "rb") as source:
        def lines():
            while raw := source.readline():
                yield raw.decode("utf-8")
        reader = csv.reader(lines())
        next(reader)
        start = source.tell()
        for record in reader:
            end = source.tell()
            offsets[record[0]] = [start, end - start]
            start = end
    _atomic_write(positions_path(db, table), json.dumps(offsets))


def rebuild(db, catalog, table, rows):
    indexes = catalog.indexes_on(table.name)
    texts = [(i, build_text(i, rows)) for i in indexes]
    for idx, text in texts:
        _atomic_write(index_path(db, idx.name), text)
    if indexes:
        write_positions(db, table)
    else:
        positions_path(db, table).unlink(missing_ok=True)


def lookup(db, table, index, op, value):
    if value is None:
        return set()
    column = table.column(index.columns[0])
    if column.type.name == "DATE":
        value = sqltypes.coerce(value, column.type)
    entries = []
    try:
        for line in index_path(db, index.name).read_text(encoding="utf-8").splitlines():
            key, ids = line.rsplit("|", 1)
            key = json.loads(key)
            if key is None:
                continue
            if column.type.name == "DATE":
                key = sqltypes.coerce(key, column.type)
            entries.append((key, [int(i) for i in ids.split(",")]))
    except (OSError, ValueError) as exc:
        raise StorageError(f"No se pudo leer el índice '{index.name}': {exc}") from exc
    keys = [entry[0] for entry in entries]
    left, right = bisect_left(keys, value), bisect_right(keys, value)
    bounds = {"=": (left, right), "<": (0, left), "<=": (0, right),
              ">": (right, len(entries)), ">=": (left, len(entries))}
    start, end = bounds[op]
    return {rid for _, ids in entries[start:end] for rid in ids}


def _terms(expr):
    if isinstance(expr, ast.Binary) and expr.op == "AND":
        yield from _terms(expr.left)
        yield from _terms(expr.right)
    else:
        yield expr


def _literal(expr):
    return isinstance(expr, ast.Literal) or (
        isinstance(expr, ast.Unary) and expr.op in ("+", "-") and _literal(expr.operand))


def candidates(db, catalog, table, where, alias=None):
    """Intersección de predicados indexables; siempre reevaluar WHERE completo."""
    selected, used = None, []
    for term in _terms(where):
        if not isinstance(term, ast.Binary) or term.op not in ("=", "<", "<=", ">", ">="):
            continue
        col, literal, op = term.left, term.right, term.op
        if _literal(col) and isinstance(literal, ast.ColumnRef):
            col, literal = literal, col
            op = {"=": "=", "<": ">", "<=": ">=", ">": "<", ">=": "<="}[op]
        if not isinstance(col, ast.ColumnRef) or not _literal(literal):
            continue
        if col.table not in (None, alias or table.name):
            continue
        idx = catalog.index_for(table.name, col.column)
        if idx is None:
            continue
        ids = lookup(db, table, idx, op, evaluate(literal))
        selected = ids if selected is None else selected & ids
        if idx.name not in used:
            used.append(idx.name)
    return selected, ("índice " + ", ".join(used)) if used else "recorrido completo"


def read_selected(db, table, rowids):
    if not rowids:
        return []
    try:
        offsets = json.loads(positions_path(db, table).read_text(encoding="utf-8"))
        rows = []
        with open(db.table_path(table.name), "rb") as source:
            for rid in sorted(rowids):
                start, length = offsets[str(rid)]
                source.seek(start)
                record = next(csv.reader(io.StringIO(source.read(length).decode("utf-8"), newline="")))
                if int(record[0]) != rid or len(record) != len(table.columns) + 1:
                    raise ValueError("posición de fila inválida")
                rows.append(Row(rid, {c.name: sqltypes.from_text(v, c.type)
                                     for c, v in zip(table.columns, record[1:])}))
        return rows
    except (OSError, KeyError, ValueError, StopIteration) as exc:
        raise StorageError(f"No se pudo acceder a las filas indexadas de '{table.name}': {exc}") from exc


def scan(db, catalog, table, where, alias=None):
    ids, plan = candidates(db, catalog, table, where, alias)
    return (db.read_rows(table) if ids is None else read_selected(db, table, ids)), plan
