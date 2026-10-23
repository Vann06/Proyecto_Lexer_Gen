"""Tipos de datos SQL: validación, compatibilidad y paso a texto.

Punto único de toda pregunta sobre tipos. La semántica pregunta aquí si dos
tipos se pueden comparar o asignar; el ejecutor convierte con `coerce` cada
valor antes de guardarlo; el almacenamiento usa `to_text`/`from_text` para
las celdas del CSV.

Valores en memoria según el tipo de la columna:
    INT -> int · FLOAT -> float · CHAR/VARCHAR -> str · BOOLEAN -> bool
    DATE -> datetime.date · NULL -> None

El tipo en sí es `ast.DataType` (nombre + longitud): el mismo objeto que
produce el parser es el que se guarda en el catálogo.
"""

from __future__ import annotations

import re
from datetime import date

from dbms.ast import DataType

INT = DataType("INT")
FLOAT = DataType("FLOAT")
BOOLEAN = DataType("BOOLEAN")
DATE = DataType("DATE")


def varchar(n: int) -> DataType:
    return DataType("VARCHAR", n)


# Familias: qué tipos se pueden comparar entre sí.
_FAMILY = {"INT": "numeric", "FLOAT": "numeric", "CHAR": "text", "VARCHAR": "text",
           "BOOLEAN": "bool", "DATE": "date"}

_DATE_RE = re.compile(r"\d{4}-\d{2}-\d{2}")
_TYPE_RE = re.compile(r"(INT|FLOAT|CHAR|VARCHAR|BOOLEAN|DATE)(?:\((\d+)\))?")

NULL_TEXT = r"\N"


class TypeMismatch(Exception):
    """Un valor no se puede guardar en una columna de cierto tipo."""


def family(t: DataType) -> str:
    return _FAMILY[t.name]


def comparable(a: DataType, b: DataType) -> bool:
    """¿Tiene sentido `a = b`, `a < b`? INT y FLOAT se comparan entre sí."""
    return family(a) == family(b)


def assignable(target: DataType, source: DataType) -> bool:
    """¿Un valor de tipo `source` cabe en una columna `target`? La única
    conversión implícita es INT -> FLOAT; la longitud de un texto se revisa
    con el valor concreto (`coerce`), no aquí."""
    if target.name == source.name or family(target) == family(source) == "text":
        return True
    return target.name == "FLOAT" and source.name == "INT"


def parse_type(text: str) -> DataType:
    """'VARCHAR(50)' -> DataType("VARCHAR", 50). Inverso de str(DataType)."""
    m = _TYPE_RE.fullmatch(text.strip().upper())
    if m is None:
        raise ValueError(f"tipo desconocido: {text!r}")
    name, length = m.group(1), m.group(2)
    if (name in ("CHAR", "VARCHAR")) != (length is not None):
        raise ValueError(f"tipo mal formado: {text!r}")
    return DataType(name, int(length) if length is not None else None)


def _describe(value) -> str:
    if isinstance(value, bool):
        return "TRUE" if value else "FALSE"
    if isinstance(value, str):
        return f"'{value}'"
    return str(value)


def coerce(value, t: DataType):
    """Valida `value` contra el tipo `t` y lo devuelve en su forma en memoria.
    NULL pasa siempre: NOT NULL es una restricción de la columna, no del tipo.
    Lanza TypeMismatch con un mensaje listo para mostrar."""
    if value is None:
        return None
    name = t.name
    # bool es subclase de int en Python: hay que excluirlo a mano.
    if name == "INT":
        if isinstance(value, int) and not isinstance(value, bool):
            return value
    elif name == "FLOAT":
        if isinstance(value, (int, float)) and not isinstance(value, bool):
            return float(value)
    elif name in ("CHAR", "VARCHAR"):
        if isinstance(value, str):
            if len(value) > t.length:
                raise TypeMismatch(f"{_describe(value)} tiene {len(value)} caracteres; {t} admite a lo sumo {t.length}")
            return value
    elif name == "BOOLEAN":
        if isinstance(value, bool):
            return value
    elif name == "DATE":
        if isinstance(value, date):
            return value
        if isinstance(value, str):
            if not _DATE_RE.fullmatch(value):
                raise TypeMismatch(f"{_describe(value)} no es una fecha con formato YYYY-MM-DD")
            try:
                return date.fromisoformat(value)
            except ValueError:
                raise TypeMismatch(f"{_describe(value)} no es una fecha válida") from None
    raise TypeMismatch(f"{_describe(value)} no es un valor de tipo {t}")


# ───────────────────────────── Texto (celdas del CSV) ─────────────────────────────
#
# NULL se escribe \N (convención de MySQL/PostgreSQL), para no confundirlo con
# la cadena vacía. Una cadena que empieza con barra invertida se escribe con
# una barra extra, así la cadena literal "\N" no se lee como NULL.

def to_text(value, t: DataType) -> str:
    if value is None:
        return NULL_TEXT
    if t.name == "BOOLEAN":
        return "TRUE" if value else "FALSE"
    if t.name == "DATE":
        return value.isoformat()
    if t.name == "FLOAT":
        return repr(float(value))
    if t.name in ("CHAR", "VARCHAR"):
        return "\\" + value if value.startswith("\\") else value
    return str(value)


def from_text(text: str, t: DataType):
    if text == NULL_TEXT:
        return None
    name = t.name
    if name == "INT":
        return int(text)
    if name == "FLOAT":
        return float(text)
    if name == "BOOLEAN":
        if text not in ("TRUE", "FALSE"):
            raise ValueError(f"booleano inválido: {text!r}")
        return text == "TRUE"
    if name == "DATE":
        return date.fromisoformat(text)
    return text[1:] if text.startswith("\\\\") else text
