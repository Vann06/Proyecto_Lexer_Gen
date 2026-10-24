"""Contrato público del motor con la API y sus extensiones."""
from dataclasses import dataclass, field
from datetime import date

from dbms.ast import Pos
from dbms.frontend.parse import Problem


@dataclass
class Result:
    columns: list[str] = field(default_factory=list)
    rows: list[list] = field(default_factory=list)
    message: str = ""
    plan: str | None = None

    def to_dict(self):
        return {"columns": self.columns,
                "rows": [[v.isoformat() if isinstance(v, date) else v for v in row]
                         for row in self.rows],
                "message": self.message, "plan": self.plan}


class ExecutionError(Exception):
    def __init__(self, code: str, message: str, pos: Pos | None = None):
        super().__init__(message)
        self.code, self.pos = code, pos

    def to_problem(self):
        return Problem("err", self.code, str(self),
                       self.pos.line if self.pos else None,
                       self.pos.col if self.pos else None)
