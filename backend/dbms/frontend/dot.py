"""Árbol de derivación de ANTLR -> Graphviz DOT (lo dibuja la pestaña
ÁRBOL SINTÁCTICO del IDE con viz.js)."""

from __future__ import annotations

from antlr4 import ParserRuleContext
from antlr4.tree.Tree import ErrorNode, TerminalNode

from dbms.frontend.parse import ParseResult


def _esc(text: str) -> str:
    return text.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n")


def _rule_label(ctx: ParserRuleContext, rule_names: list[str]) -> str:
    """Nombre de la regla, y el de la alternativa etiquetada si la hay
    (`predicate` + ComparePredContext -> "predicate · comparePred")."""
    rule = rule_names[ctx.getRuleIndex()]
    cls = type(ctx).__name__.removesuffix("Context")
    alt = cls[0].lower() + cls[1:]
    return rule if alt == rule else f"{rule} · {alt}"


def tree_to_dot(result: ParseResult) -> str:
    rule_names = result.parser.ruleNames
    lines = [
        "digraph ParseTree {",
        '  node [shape=box, style="rounded,filled", fontname="monospace", fontsize=11, fillcolor="#2b1f3a", fontcolor="#f5e6ff", color="#b35cff"];',
        '  edge [color="#8a6fb0"];',
    ]
    counter = 0

    def visit(node) -> str:
        nonlocal counter
        nid = f"n{counter}"
        counter += 1
        if isinstance(node, ErrorNode):
            lines.append(f'  {nid} [label="{_esc(node.getText())}", shape=ellipse, fillcolor="#5c1a2a", color="#ff4d6d"];')
        elif isinstance(node, TerminalNode):
            lines.append(f'  {nid} [label="{_esc(node.getText())}", shape=ellipse, fillcolor="#123a3a", color="#3de0d0"];')
        else:
            lines.append(f'  {nid} [label="{_esc(_rule_label(node, rule_names))}"];')
            for child in node.getChildren():
                lines.append(f"  {nid} -> {visit(child)};")
        return nid

    visit(result.tree)
    lines.append("}")
    return "\n".join(lines)
