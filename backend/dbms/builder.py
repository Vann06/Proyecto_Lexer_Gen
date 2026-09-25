"""Árbol de ANTLR -> AST (`dbms.ast`).

Un método `visitX` por regla o alternativa etiquetada de grammar/SQL.g4.
Solo se llama sobre árboles sin errores de sintaxis (ver `build`): con
recuperación de errores ANTLR deja hijos faltantes, y traducir un árbol
incompleto a un AST solo movería el problema a la fase siguiente.
"""

from __future__ import annotations

from antlr4 import ParserRuleContext

from dbms import ast
from dbms.frontend.parse import ParseResult
from dbms.parser.SQLParser import SQLParser
from dbms.parser.SQLVisitor import SQLVisitor


class SyntaxErrorsPresent(Exception):
    """Se pidió construir el AST de un script con errores de sintaxis."""


def build(result: ParseResult) -> list[ast.Statement]:
    if not result.ok:
        raise SyntaxErrorsPresent("el script tiene errores de sintaxis; no se construye el AST")
    return ASTBuilder(result.text).visit(result.tree)


def _pos(ctx: ParserRuleContext) -> ast.Pos:
    return ast.Pos(ctx.start.line, ctx.start.column + 1)


class ASTBuilder(SQLVisitor):
    def __init__(self, source: str):
        self.source = source

    def _text(self, ctx: ParserRuleContext) -> str:
        """El texto original de `ctx`, con sus espacios y comentarios."""
        return self.source[ctx.start.start:ctx.stop.stop + 1]

    # ───────────── Script ─────────────

    def visitScript(self, ctx: SQLParser.ScriptContext) -> list[ast.Statement]:
        return [ast.Statement(body=self.visit(s), sql=self._text(s), pos=_pos(s)) for s in ctx.sqlStmt()]

    def visitSqlStmt(self, ctx: SQLParser.SqlStmtContext):
        return self.visit(ctx.getChild(0))

    # ───────────── Identificadores y listas ─────────────

    def visitIdentifier(self, ctx: SQLParser.IdentifierContext) -> str:
        quoted = ctx.QUOTED_IDENTIFIER()
        if quoted is not None:
            text = quoted.getText()
            inner = text[1:-1]
            return inner.replace('""', '"') if text[0] == '"' else inner
        return ctx.getText().lower()

    def visitColumnList(self, ctx: SQLParser.ColumnListContext) -> list[str]:
        return [self.visit(i) for i in ctx.identifier()]

    # ───────────── Bases de datos ─────────────

    def visitCreateDatabase(self, ctx):
        return ast.CreateDatabase(self.visit(ctx.identifier()), if_not_exists=ctx.IF() is not None)

    def visitDropDatabase(self, ctx):
        return ast.DropDatabase(self.visit(ctx.identifier()), if_exists=ctx.IF() is not None)

    def visitAlterDatabase(self, ctx):
        return ast.AlterDatabase(self.visit(ctx.old), self.visit(ctx.new))

    def visitUseDatabase(self, ctx):
        return ast.UseDatabase(self.visit(ctx.identifier()))

    def visitShowDatabases(self, ctx):
        return ast.ShowDatabases()

    def visitShowTables(self, ctx):
        return ast.ShowTables()

    def visitShowColumns(self, ctx):
        return ast.ShowColumns(self.visit(ctx.identifier()))

    # ───────────── Tablas ─────────────

    def visitCreateTable(self, ctx: SQLParser.CreateTableContext):
        columns: list[ast.ColumnDef] = []
        constraints: list[ast.Constraint] = []
        for element in ctx.tableElement():
            if element.columnDef() is not None:
                column, col_constraints = self.visit(element.columnDef())
                columns.append(column)
                constraints.extend(col_constraints)
            else:
                constraints.append(self.visit(element.tableConstraint()))
        return ast.CreateTable(self.visit(ctx.identifier()), columns, constraints,
                               if_not_exists=ctx.IF() is not None)

    def visitColumnDef(self, ctx: SQLParser.ColumnDefContext) -> tuple[ast.ColumnDef, list[ast.Constraint]]:
        """Devuelve la columna y, aparte, las restricciones escritas junto a
        ella ya convertidas a su forma de tabla."""
        name = self.visit(ctx.identifier())
        not_null = False
        default = None
        constraints: list[ast.Constraint] = []
        for cc in ctx.columnConstraint():
            cname = self.visit(cc.cname) if getattr(cc, "cname", None) is not None else None
            pos = _pos(cc)
            if isinstance(cc, SQLParser.ColNotNullContext):
                not_null = True
            elif isinstance(cc, SQLParser.ColNullContext):
                not_null = False
            elif isinstance(cc, SQLParser.ColDefaultContext):
                default = self.visit(cc.valueExpr())
            elif isinstance(cc, SQLParser.ColPrimaryKeyContext):
                constraints.append(ast.PrimaryKey(cname, [name], pos))
            elif isinstance(cc, SQLParser.ColUniqueContext):
                constraints.append(ast.Unique(cname, [name], pos))
            elif isinstance(cc, SQLParser.ColCheckContext):
                constraints.append(ast.Check(cname, self.visit(cc.expr()), self._text(cc.expr()), pos))
            elif isinstance(cc, SQLParser.ColReferencesContext):
                ref_cols = [self.visit(cc.refCol)] if cc.refCol is not None else []
                constraints.append(ast.ForeignKey(cname, [name], self.visit(cc.ref), ref_cols, pos))
        column = ast.ColumnDef(name, self.visit(ctx.dataType()), not_null, default, _pos(ctx))
        return column, constraints

    # Tipos
    def visitIntType(self, ctx):
        return ast.DataType("INT")

    def visitFloatType(self, ctx):
        return ast.DataType("FLOAT")

    def visitCharType(self, ctx):
        return ast.DataType("CHAR", int(ctx.INT_LIT().getText()))

    def visitVarcharType(self, ctx):
        return ast.DataType("VARCHAR", int(ctx.INT_LIT().getText()))

    def visitDateType(self, ctx):
        return ast.DataType("DATE")

    def visitBoolType(self, ctx):
        return ast.DataType("BOOLEAN")

    # Restricciones de tabla
    def _cname(self, ctx) -> str | None:
        return self.visit(ctx.cname) if ctx.cname is not None else None

    def visitTcPrimaryKey(self, ctx):
        return ast.PrimaryKey(self._cname(ctx), self.visit(ctx.columnList()), _pos(ctx))

    def visitTcUnique(self, ctx):
        return ast.Unique(self._cname(ctx), self.visit(ctx.columnList()), _pos(ctx))

    def visitTcForeignKey(self, ctx):
        ref_cols = self.visit(ctx.refCols) if ctx.refCols is not None else []
        return ast.ForeignKey(self._cname(ctx), self.visit(ctx.cols), self.visit(ctx.ref), ref_cols, _pos(ctx))

    def visitTcCheck(self, ctx):
        return ast.Check(self._cname(ctx), self.visit(ctx.expr()), self._text(ctx.expr()), _pos(ctx))

    def visitDropTable(self, ctx):
        return ast.DropTable(self.visit(ctx.identifier()), if_exists=ctx.IF() is not None)

    # ALTER TABLE
    def visitAlterTable(self, ctx: SQLParser.AlterTableContext):
        return ast.AlterTable(self.visit(ctx.identifier()), [self.visit(a) for a in ctx.alterAction()])

    def visitRenameTable(self, ctx):
        return ast.RenameTable(self.visit(ctx.identifier()), _pos(ctx))

    def visitRenameColumn(self, ctx):
        return ast.RenameColumn(self.visit(ctx.old), self.visit(ctx.new), _pos(ctx))

    def visitAddColumn(self, ctx):
        column, constraints = self.visit(ctx.columnDef())
        return ast.AddColumn(column, constraints, _pos(ctx))

    def visitDropColumn(self, ctx):
        return ast.DropColumn(self.visit(ctx.identifier()), _pos(ctx))

    def visitAddConstraint(self, ctx):
        return ast.AddConstraint(self.visit(ctx.tableConstraint()), _pos(ctx))

    def visitDropConstraint(self, ctx):
        return ast.DropConstraint(self.visit(ctx.identifier()), _pos(ctx))

    def visitAlterColumnType(self, ctx):
        return ast.AlterColumnType(self.visit(ctx.identifier()), self.visit(ctx.dataType()), _pos(ctx))

    # ───────────── Índices ─────────────

    def visitCreateIndex(self, ctx):
        return ast.CreateIndex(self.visit(ctx.idx), self.visit(ctx.tbl), self.visit(ctx.columnList()),
                               unique=ctx.UNIQUE() is not None, if_not_exists=ctx.IF() is not None)

    def visitDropIndex(self, ctx):
        return ast.DropIndex(self.visit(ctx.identifier()), if_exists=ctx.IF() is not None)

    # ───────────── DML ─────────────

    def visitInsertStmt(self, ctx: SQLParser.InsertStmtContext):
        columns = self.visit(ctx.columnList()) if ctx.columnList() is not None else None
        rows = [[self.visit(e) for e in row.expr()] for row in ctx.valuesRow()]
        return ast.Insert(self.visit(ctx.identifier()), columns, rows)

    def visitUpdateStmt(self, ctx: SQLParser.UpdateStmtContext):
        assignments = [ast.Assignment(self.visit(a.identifier()), self.visit(a.expr()), _pos(a))
                       for a in ctx.assignment()]
        return ast.Update(self.visit(ctx.identifier()), assignments, self._where(ctx.whereClause()))

    def visitDeleteStmt(self, ctx: SQLParser.DeleteStmtContext):
        return ast.Delete(self.visit(ctx.identifier()), self._where(ctx.whereClause()))

    def _where(self, ctx) -> ast.Expr | None:
        return self.visit(ctx.expr()) if ctx is not None else None

    # ───────────── SELECT ─────────────

    def visitSelectStmt(self, ctx: SQLParser.SelectStmtContext):
        select = ast.Select(distinct=ctx.DISTINCT() is not None,
                            items=self.visit(ctx.selectList()),
                            from_table=self.visit(ctx.tableRef()) if ctx.tableRef() is not None else None,
                            joins=[self.visit(j) for j in ctx.joinClause()],
                            where=self._where(ctx.whereClause()))
        if ctx.groupByClause() is not None:
            select.group_by = [self.visit(e) for e in ctx.groupByClause().expr()]
        if ctx.havingClause() is not None:
            select.having = self.visit(ctx.havingClause().expr())
        if ctx.orderByClause() is not None:
            select.order_by = [ast.OrderItem(self.visit(o.expr()), descending=o.DESC() is not None)
                               for o in ctx.orderByClause().orderItem()]
        if ctx.limitClause() is not None:
            lc = ctx.limitClause()
            select.limit = int(lc.limit.text)
            select.offset = int(lc.offset.text) if lc.offset is not None else None
        return select

    def visitSelectList(self, ctx: SQLParser.SelectListContext) -> list[ast.SelectItem]:
        if ctx.STAR() is not None:
            return [ast.Star(None, _pos(ctx))]
        return [self.visit(i) for i in ctx.selectItem()]

    def visitTableStarItem(self, ctx):
        return ast.Star(self.visit(ctx.identifier()), _pos(ctx))

    def visitExprItem(self, ctx):
        alias = self.visit(ctx.alias) if ctx.alias is not None else None
        return ast.ExprItem(self.visit(ctx.expr()), alias, self._text(ctx.expr()), _pos(ctx))

    def visitTableRef(self, ctx):
        alias = self.visit(ctx.alias) if ctx.alias is not None else None
        return ast.TableRef(self.visit(ctx.tbl), alias, _pos(ctx))

    def visitInnerJoin(self, ctx):
        return ast.Join("INNER", self.visit(ctx.tableRef()), self.visit(ctx.expr()), _pos(ctx))

    def visitLeftJoin(self, ctx):
        return ast.Join("LEFT", self.visit(ctx.tableRef()), self.visit(ctx.expr()), _pos(ctx))

    def visitRightJoin(self, ctx):
        return ast.Join("RIGHT", self.visit(ctx.tableRef()), self.visit(ctx.expr()), _pos(ctx))

    def visitCrossJoin(self, ctx):
        return ast.Join("CROSS", self.visit(ctx.tableRef()), None, _pos(ctx))

    # ───────────── Expresiones: lógica ─────────────

    def visitNotExpr(self, ctx):
        return ast.Unary("NOT", self.visit(ctx.expr()), _pos(ctx))

    def visitAndExpr(self, ctx):
        return ast.Binary("AND", self.visit(ctx.expr(0)), self.visit(ctx.expr(1)), _pos(ctx))

    def visitOrExpr(self, ctx):
        return ast.Binary("OR", self.visit(ctx.expr(0)), self.visit(ctx.expr(1)), _pos(ctx))

    def visitPredicateExpr(self, ctx):
        return self.visit(ctx.predicate())

    # ───────────── Expresiones: predicados ─────────────

    def visitComparePred(self, ctx):
        op = "<>" if ctx.op.text == "!=" else ctx.op.text
        return ast.Binary(op, self.visit(ctx.valueExpr(0)), self.visit(ctx.valueExpr(1)), _pos(ctx))

    def visitIsNullPred(self, ctx):
        return ast.IsNull(self.visit(ctx.valueExpr()), negated=ctx.NOT() is not None, pos=_pos(ctx))

    def visitInPred(self, ctx):
        operand, *items = [self.visit(v) for v in ctx.valueExpr()]
        return ast.InList(operand, items, negated=ctx.NOT() is not None, pos=_pos(ctx))

    def visitLikePred(self, ctx):
        return ast.Like(self.visit(ctx.valueExpr(0)), self.visit(ctx.valueExpr(1)),
                        negated=ctx.NOT() is not None, pos=_pos(ctx))

    def visitBetweenPred(self, ctx):
        return ast.Between(self.visit(ctx.valueExpr(0)), self.visit(ctx.low), self.visit(ctx.high),
                           negated=ctx.NOT() is not None, pos=_pos(ctx))

    def visitValuePred(self, ctx):
        return self.visit(ctx.valueExpr())

    # ───────────── Expresiones: valores ─────────────

    def visitUnaryExpr(self, ctx):
        operand = self.visit(ctx.valueExpr())
        # `-5` es un literal negativo, no una operación: así `DEFAULT -1` y
        # `VALUES (-3)` siguen siendo constantes para la fase semántica.
        if isinstance(operand, ast.Literal) and operand.kind in ("int", "float"):
            value = -operand.value if ctx.op.text == "-" else operand.value
            return ast.Literal(value, operand.kind, _pos(ctx))
        return ast.Unary(ctx.op.text, operand, _pos(ctx))

    def visitMulExpr(self, ctx):
        return ast.Binary(ctx.op.text, self.visit(ctx.valueExpr(0)), self.visit(ctx.valueExpr(1)), _pos(ctx))

    def visitAddExpr(self, ctx):
        return ast.Binary(ctx.op.text, self.visit(ctx.valueExpr(0)), self.visit(ctx.valueExpr(1)), _pos(ctx))

    def visitParenExpr(self, ctx):
        return self.visit(ctx.expr())

    def visitFuncExpr(self, ctx):
        return self.visit(ctx.functionCall())

    def visitColumnExpr(self, ctx):
        return self.visit(ctx.columnRef())

    def visitLiteralExpr(self, ctx):
        return self.visit(ctx.literal())

    def visitFunctionCall(self, ctx: SQLParser.FunctionCallContext):
        return ast.FuncCall(name=self.visit(ctx.fname).upper(),
                            args=[self.visit(e) for e in ctx.expr()],
                            star=ctx.STAR() is not None,
                            distinct=ctx.DISTINCT() is not None,
                            pos=_pos(ctx))

    def visitColumnRef(self, ctx):
        table = self.visit(ctx.tbl) if ctx.tbl is not None else None
        return ast.ColumnRef(table, self.visit(ctx.col), _pos(ctx))

    # Literales
    def visitIntLiteral(self, ctx):
        return ast.Literal(int(ctx.getText()), "int", _pos(ctx))

    def visitFloatLiteral(self, ctx):
        return ast.Literal(float(ctx.getText()), "float", _pos(ctx))

    def visitStringLiteral(self, ctx):
        return ast.Literal(ctx.getText()[1:-1].replace("''", "'"), "string", _pos(ctx))

    def visitBoolLiteral(self, ctx):
        return ast.Literal(ctx.TRUE() is not None, "bool", _pos(ctx))

    def visitNullLiteral(self, ctx):
        return ast.Literal(None, "null", _pos(ctx))
