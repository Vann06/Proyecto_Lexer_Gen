"""AST (`dbms.ast`) -> Graphviz DOT (lo dibuja la pestaña AST del IDE).

El recorrido es genérico sobre los dataclasses del AST, no un caso por nodo:
un campo que contiene nodos (o listas de nodos) es una arista rotulada con
el nombre del campo; el resto de campos con valor va en la etiqueta.
Así un nodo nuevo en `dbms.ast` aparece en el dibujo sin tocar este archivo.
"""

from __future__ import annotations

from dataclasses import fields, is_dataclass

from dbms import ast
from dbms.frontend.dot import _esc

# Campos que no aportan al dibujo: la posición y el texto original.
_HIDDEN = {"pos", "sql", "text"}
# Dataclasses que se muestran como valor (en la etiqueta), no como nodo.
_SCALARS = (ast.DataType,)


def _is_node(value) -> bool:
    return is_dataclass(value) and not isinstance(value, type) and not isinstance(value, _SCALARS)


def _has_nodes(value) -> bool:
    if isinstance(value, list):
        return any(_has_nodes(v) for v in value)
    return _is_node(value)


def _show(name: str, value) -> str | None:
    """Texto de un campo escalar para la etiqueta, o None si no se muestra."""
    if name == "value" and isinstance(value, bool):     # el de un Literal: FALSE también cuenta
        return f"value: {str(value).upper()}"
    if value is None or value is False or value == [] or value == "":
        return None
    if value is True:
        return name
    if isinstance(value, list):
        return f"{name}: {', '.join(map(str, value))}"
    if name == "value" and isinstance(value, str):
        return f"value: '{value}'"
    return f"{name}: {value}"


def ast_to_dot(statements: list[ast.Statement]) -> str:
    lines = [
        "digraph AST {",
        '  node [shape=box, style="rounded,filled", fontname="monospace", fontsize=11, fillcolor="#12303a", fontcolor="#e6fbff", color="#3de0d0"];',
        '  edge [color="#5fa8b0", fontname="monospace", fontsize=9, fontcolor="#9fd8de"];',
    ]
    counter = 0

    def new_id() -> str:
        nonlocal counter
        nid = f"a{counter}"
        counter += 1
        return nid

    def edges(parent: str, label: str, value) -> None:
        if isinstance(value, list):
            for i, item in enumerate(value):
                edges(parent, f"{label}[{i}]", item)
        elif _is_node(value):
            lines.append(f'  {parent} -> {visit(value)} [label="{_esc(label)}"];')

    def visit(node) -> str:
        nid = new_id()
        label = [type(node).__name__]
        children = []
        for f in fields(node):
            if f.name in _HIDDEN:
                continue
            value = getattr(node, f.name)
            if _has_nodes(value):
                children.append((f.name, value))
            elif (text := _show(f.name, value)) is not None:
                label.append(text)
        lines.append(f'  {nid} [label="{_esc(chr(10).join(label))}"];')
        for name, value in children:
            edges(nid, name, value)
        return nid

    root = new_id()
    lines.append(f'  {root} [label="Script", fillcolor="#2b1f3a", color="#b35cff"];')
    for i, stmt in enumerate(statements):
        lines.append(f'  {root} -> {visit(stmt.body)} [label="{i + 1}"];')
    lines.append("}")
    return "\n".join(lines)
