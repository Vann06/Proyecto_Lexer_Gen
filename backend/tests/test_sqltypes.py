"""Fase B: tipos de datos."""

import re
from datetime import date

import pytest

from dbms import sqltypes as T
from dbms.ast import DataType

V5 = T.varchar(5)


@pytest.mark.parametrize("value, t, expected", [
    (5, T.INT, 5),
    (-3, T.INT, -3),
    (5, T.FLOAT, 5.0),               # INT -> FLOAT implícito
    (2.5, T.FLOAT, 2.5),
    ("hola", V5, "hola"),
    ("", V5, ""),
    ("ab", DataType("CHAR", 2), "ab"),
    (True, T.BOOLEAN, True),
    (False, T.BOOLEAN, False),
    ("2024-02-29", T.DATE, date(2024, 2, 29)),
    (date(2024, 1, 1), T.DATE, date(2024, 1, 1)),
    (None, T.INT, None),             # NULL pasa: NOT NULL es de la columna
])
def test_coerce_acepta(value, t, expected):
    result = T.coerce(value, t)
    assert result == expected and type(result) is type(expected)


@pytest.mark.parametrize("value, t, fragment", [
    (5.5, T.INT, "no es un valor de tipo INT"),
    ("5", T.INT, "no es un valor de tipo INT"),
    (True, T.INT, "TRUE no es un valor de tipo INT"),     # bool no cuenta como int
    (True, T.FLOAT, "no es un valor de tipo FLOAT"),
    ("abcdef", V5, "tiene 6 caracteres; VARCHAR(5) admite a lo sumo 5"),
    (3, V5, "no es un valor de tipo VARCHAR(5)"),
    (1, T.BOOLEAN, "no es un valor de tipo BOOLEAN"),
    ("TRUE", T.BOOLEAN, "no es un valor de tipo BOOLEAN"),
    ("2024-02-30", T.DATE, "no es una fecha válida"),
    ("15/03/2024", T.DATE, "formato YYYY-MM-DD"),
    ("20240315", T.DATE, "formato YYYY-MM-DD"),
    (20240315, T.DATE, "no es un valor de tipo DATE"),
])
def test_coerce_rechaza(value, t, fragment):
    with pytest.raises(T.TypeMismatch, match=re.escape(fragment)):
        T.coerce(value, t)


def test_compatibilidad():
    assert T.comparable(T.INT, T.FLOAT)
    assert T.comparable(T.varchar(3), DataType("CHAR", 9))
    assert not T.comparable(T.DATE, T.varchar(10))
    assert not T.comparable(T.BOOLEAN, T.INT)
    assert T.assignable(T.FLOAT, T.INT)
    assert not T.assignable(T.INT, T.FLOAT)
    assert T.assignable(T.varchar(2), T.varchar(200))      # la longitud se revisa con el valor
    assert not T.assignable(T.DATE, T.varchar(10))


@pytest.mark.parametrize("text, t", [
    ("INT", T.INT), ("varchar(50)", T.varchar(50)), (" DATE ", T.DATE), ("CHAR(1)", DataType("CHAR", 1)),
])
def test_parse_type(text, t):
    assert T.parse_type(text) == t
    assert T.parse_type(str(t)) == t


@pytest.mark.parametrize("text", ["TEXT", "VARCHAR", "INT(3)", "VARCHAR(x)"])
def test_parse_type_invalido(text):
    with pytest.raises(ValueError):
        T.parse_type(text)


@pytest.mark.parametrize("value, t, text", [
    (None, T.INT, r"\N"),
    (7, T.INT, "7"),
    (0.1, T.FLOAT, "0.1"),
    (3.0, T.FLOAT, "3.0"),
    (True, T.BOOLEAN, "TRUE"),
    (False, T.BOOLEAN, "FALSE"),
    (date(2024, 3, 5), T.DATE, "2024-03-05"),
    ("", V5, ""),
    (r"\N", V5, r"\\N"),             # la cadena "\N" no es NULL
    ("\\x", V5, "\\\\x"),
])
def test_texto_ida_y_vuelta(value, t, text):
    assert T.to_text(value, t) == text
    assert T.from_text(text, t) == value


def test_from_text_booleano_invalido():
    with pytest.raises(ValueError):
        T.from_text("yes", T.BOOLEAN)
