"""Análisis léxico y sintáctico con ANTLR: texto SQL -> árbol + problemas.

Los mensajes de error de ANTLR vienen en inglés y con la redacción interna
del runtime ("mismatched input ... expecting ..."). Aquí se traducen a
`Problem`, el mismo contrato JSON que consume el panel PROBLEMAS del IDE
({level, code, msg, loc, line, col}).
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

from antlr4 import CommonTokenStream, InputStream, Token
from antlr4.error.ErrorListener import ErrorListener

from dbms.parser.SQLLexer import SQLLexer
from dbms.parser.SQLParser import SQLParser


@dataclass
class Problem:
    level: str          # "err" | "warn" | "info"
    code: str           # LEXnnn, SYNnnn, SEMnnn, EXEnnn
    msg: str
    line: int | None = None
    col: int | None = None   # 1-based, como lo muestra el editor

    @property
    def loc(self) -> str:
        return f"{self.line}:{self.col}" if self.line is not None else ""

    def to_dict(self) -> dict:
        return {"level": self.level, "code": self.code, "msg": self.msg,
                "loc": self.loc, "line": self.line, "col": self.col}


@dataclass
class ParseResult:
    text: str
    tree: SQLParser.ScriptContext
    parser: SQLParser
    tokens: list[Token]
    problems: list[Problem] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not any(p.level == "err" for p in self.problems)


# Nombres legibles para los tokens que no son palabras clave.
_TOKEN_NAMES = {
    "IDENTIFIER": "identificador", "INT_LIT": "entero", "FLOAT_LIT": "decimal",
    "STRING_LIT": "cadena", "fin de archivo": "fin de archivo",
}
# Palabras clave que la gramática también acepta como identificador
# (regla nonReserved): en "se esperaba" solo repiten "identificador".
_REDUNDANT = {"QUOTED_IDENTIFIER", "'COLUMNS'", "'DATA'", "'DATABASES'", "'DATE'", "'TABLES'", "'TYPE'"}
_MAX_EXPECTED = 8
# Un elemento del conjunto: literal entre comillas (puede ser ',') o nombre de token.
_EXPECTED_ITEM = re.compile(r"'(?:[^']|'')*'|[^,\s]+")


def _expected(text: str) -> str:
    """"{'CHECK', IDENTIFIER, ...}" -> "'CHECK', identificador, ..." sin ruido."""
    text = text.strip()
    items = _EXPECTED_ITEM.findall(text[1:-1]) if text.startswith("{") else [text]
    if "IDENTIFIER" in items:
        items = [i for i in items if i not in _REDUNDANT]
    items = [_TOKEN_NAMES.get(i, i) for i in items]
    if len(items) > _MAX_EXPECTED:
        return ", ".join(items[:_MAX_EXPECTED]) + ", …"
    return items[0] if len(items) == 1 else "uno de: " + ", ".join(items)


def _translate(msg: str, offending: str | None) -> tuple[str, str]:
    msg = msg.replace("'<EOF>'", "fin de archivo").replace("<EOF>", "fin de archivo")
    m = re.match(r"^mismatched input (.+?) expecting (.+)$", msg, re.S)
    if m:
        return "SYN001", f"Entrada inesperada {m.group(1)}; se esperaba {_expected(m.group(2))}"
    m = re.match(r"^extraneous input (.+?) expecting (.+)$", msg, re.S)
    if m:
        return "SYN002", f"Entrada sobrante {m.group(1)}; se esperaba {_expected(m.group(2))}"
    m = re.match(r"^missing (.+?) at (.+)$", msg, re.S)
    if m:
        return "SYN003", f"Falta {_expected(m.group(1))} antes de {m.group(2)}"
    if msg.startswith("no viable alternative"):
        # ANTLR pega los lexemas sin espacios ('CREATETABLA'); se cita solo
        # el token donde se detectó el error.
        return "SYN004", f"Sentencia no válida cerca de '{offending}'"
    return "SYN000", msg


class _LexerErrors(ErrorListener):
    def __init__(self, problems: list[Problem]):
        self.problems = problems

    def syntaxError(self, recognizer, offendingSymbol, line, column, msg, e):
        m = re.match(r"^token recognition error at: '(.*)'$", msg, re.S)
        char = m.group(1) if m else msg
        self.problems.append(Problem("err", "LEX001", f"Carácter no reconocido '{char}'", line, column + 1))


class _ParserErrors(ErrorListener):
    def __init__(self, problems: list[Problem]):
        self.problems = problems

    def syntaxError(self, recognizer, offendingSymbol, line, column, msg, e):
        # Una cadena sin cerrar ya se reportó como LEX002; el parser siempre
        # tropieza con ese mismo token y el segundo error solo agrega ruido.
        if offendingSymbol is not None and offendingSymbol.type == SQLLexer.UNTERMINATED_STRING:
            return
        offending = offendingSymbol.text if offendingSymbol is not None else None
        if offending == "<EOF>":
            offending = "fin de archivo"
        code, text = _translate(msg, offending)
        self.problems.append(Problem("err", code, text, line, column + 1))


def parse(text: str) -> ParseResult:
    """Tokeniza y parsea un script SQL completo. Nunca lanza por errores de
    sintaxis: ANTLR se recupera y los errores quedan en `problems`."""
    problems: list[Problem] = []

    lexer = SQLLexer(InputStream(text))
    lexer.removeErrorListeners()
    lexer.addErrorListener(_LexerErrors(problems))

    stream = CommonTokenStream(lexer)
    stream.fill()
    for tok in stream.tokens:
        if tok.type == SQLLexer.UNTERMINATED_STRING:
            problems.append(Problem("err", "LEX002", "Cadena sin cerrar: falta la comilla simple final",
                                    tok.line, tok.column + 1))

    parser = SQLParser(stream)
    parser.removeErrorListeners()
    parser.addErrorListener(_ParserErrors(problems))
    tree = parser.script()

    tokens = [t for t in stream.tokens if t.type != Token.EOF]
    return ParseResult(text=text, tree=tree, parser=parser, tokens=tokens, problems=problems)


def tokens_to_json(result: ParseResult) -> list[dict]:
    """Formato de la pestaña TOKENS: [{kind, lexeme, line, col}]."""
    names = result.parser.symbolicNames
    return [{"kind": names[t.type] if 0 < t.type < len(names) else str(t.type),
             "lexeme": t.text, "line": t.line, "col": t.column + 1}
            for t in result.tokens]
