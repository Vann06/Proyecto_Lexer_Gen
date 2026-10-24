"""Expresiones SQL y ámbitos, del más cercano al más externo."""
from dataclasses import dataclass, field
from datetime import date
import operator
import re
from typing import Callable

from dbms import ast
from dbms.result import ExecutionError


@dataclass
class Environment:
    scopes: list[dict[str, dict[str, object]]] = field(default_factory=list)
    subquery_runner: Callable | None = None
    aggregate_runner: Callable | None = None

    def push(self, scope):
        return Environment([scope, *self.scopes], self.subquery_runner, self.aggregate_runner)

    def run_subquery(self, select):
        if self.subquery_runner is None:
            raise ExecutionError("EXE099", "La ejecución de subconsultas aún no está conectada")
        return self.subquery_runner(select, self)

    def resolve(self, ref):
        for scope in self.scopes:
            if ref.table is not None:
                if ref.table in scope:
                    if ref.column in scope[ref.table]:
                        return scope[ref.table][ref.column]
                    break  # un alias local oculta al externo, incluso sin esa columna
            else:
                matches = [row[ref.column] for row in scope.values() if ref.column in row]
                if len(matches) > 1:
                    raise ExecutionError("EXE005", f"Columna ambigua: {ref.column}", ref.pos)
                if matches:
                    return matches[0]
        raise ExecutionError("EXE005", f"Columna no disponible: {ref}", ref.pos)


def sql_not(value):
    return None if value is None else not value


def sql_and(a, b):
    return False if a is False or b is False else (None if a is None or b is None else True)


def sql_or(a, b):
    return True if a is True or b is True else (None if a is None or b is None else False)


COMPARISONS = {"=": operator.eq, "<>": operator.ne, "!=": operator.ne,
               "<": operator.lt, "<=": operator.le, ">": operator.gt, ">=": operator.ge}


def compare(op, a, b):
    if a is None or b is None:
        return None
    if isinstance(a, date) and isinstance(b, str):
        b = date.fromisoformat(b)
    elif isinstance(b, date) and isinstance(a, str):
        a = date.fromisoformat(a)
    return COMPARISONS[op](a, b)


def _in(value, values):
    result = False
    for item in values:
        result = sql_or(result, compare("=", value, item))
        if result is True:
            break
    return result


def evaluate(expr, env=None):
    env = env if isinstance(env, Environment) else Environment(env or [])
    try:
        return _evaluate(expr, env)
    except ExecutionError:
        raise
    except (ArithmeticError, TypeError, ValueError) as exc:
        raise ExecutionError("EXE004", f"No se pudo evaluar la expresión: {exc}", expr.pos) from exc


def _evaluate(e, env):
    ev = lambda node: evaluate(node, env)
    if isinstance(e, ast.Literal):
        return e.value
    if isinstance(e, ast.ColumnRef):
        return env.resolve(e)
    if isinstance(e, ast.Unary):
        v = ev(e.operand)
        return sql_not(v) if e.op == "NOT" else (None if v is None else (-v if e.op == "-" else +v))
    if isinstance(e, ast.Binary):
        a = ev(e.left)
        if e.op == "AND" and a is False:
            return False
        if e.op == "OR" and a is True:
            return True
        b = ev(e.right)
        if e.op == "AND":
            return sql_and(a, b)
        if e.op == "OR":
            return sql_or(a, b)
        if e.op in COMPARISONS:
            return compare(e.op, a, b)
        if a is None or b is None:
            return None
        # La semántica infiere INT / INT como INT: truncar hacia cero.
        if e.op == "/" and type(a) is int and type(b) is int:
            return (abs(a) // abs(b)) * (-1 if (a < 0) != (b < 0) else 1)
        if e.op == "%" and type(a) is int and type(b) is int:
            quotient = (abs(a) // abs(b)) * (-1 if (a < 0) != (b < 0) else 1)
            return a - quotient * b
        operations = {"+": operator.add, "-": operator.sub, "*": operator.mul,
                      "/": operator.truediv, "%": operator.mod, "||": operator.add}
        return operations[e.op](a, b)
    if isinstance(e, ast.IsNull):
        value = ev(e.operand) is None
        return not value if e.negated else value
    if isinstance(e, ast.InList):
        value = _in(ev(e.operand), (ev(item) for item in e.items))
        return sql_not(value) if e.negated else value
    if isinstance(e, ast.Between):
        v = ev(e.operand)
        value = sql_and(compare(">=", v, ev(e.low)), compare("<=", v, ev(e.high)))
        return sql_not(value) if e.negated else value
    if isinstance(e, ast.Like):
        value, pattern = ev(e.operand), ev(e.pattern)
        if value is None or pattern is None:
            return None
        regex = "".join(".*" if c == "%" else "." if c == "_" else re.escape(c) for c in pattern)
        matched = re.fullmatch(regex, value, re.DOTALL) is not None
        return not matched if e.negated else matched
    if isinstance(e, ast.FuncCall) and env.aggregate_runner:
        return env.aggregate_runner(e, env)
    if isinstance(e, (ast.ScalarSubquery, ast.InSubquery, ast.Exists)):
        result = env.run_subquery(e.select)
        if isinstance(e, ast.Exists):
            return not bool(result.rows) if e.negated else bool(result.rows)
        if len(result.columns) != 1:
            raise ExecutionError("EXE006", "La subconsulta debe devolver una columna", e.pos)
        if isinstance(e, ast.ScalarSubquery):
            if len(result.rows) > 1:
                raise ExecutionError("EXE003", "La subconsulta escalar devolvió más de una fila", e.pos)
            return result.rows[0][0] if result.rows else None
        value = _in(ev(e.operand), (row[0] for row in result.rows))
        return sql_not(value) if e.negated else value
    raise ExecutionError("EXE099", f"Expresión aún no soportada: {type(e).__name__}", e.pos)
