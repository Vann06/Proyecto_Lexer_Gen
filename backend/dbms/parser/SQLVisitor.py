# Generated from SQL.g4 by ANTLR 4.13.2
from antlr4 import *
if "." in __name__:
    from .SQLParser import SQLParser
else:
    from SQLParser import SQLParser

# This class defines a complete generic visitor for a parse tree produced by SQLParser.

class SQLVisitor(ParseTreeVisitor):

    # Visit a parse tree produced by SQLParser#script.
    def visitScript(self, ctx:SQLParser.ScriptContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#sqlStmt.
    def visitSqlStmt(self, ctx:SQLParser.SqlStmtContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#createDatabase.
    def visitCreateDatabase(self, ctx:SQLParser.CreateDatabaseContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#dropDatabase.
    def visitDropDatabase(self, ctx:SQLParser.DropDatabaseContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#alterDatabase.
    def visitAlterDatabase(self, ctx:SQLParser.AlterDatabaseContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#useDatabase.
    def visitUseDatabase(self, ctx:SQLParser.UseDatabaseContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#showDatabases.
    def visitShowDatabases(self, ctx:SQLParser.ShowDatabasesContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#showTables.
    def visitShowTables(self, ctx:SQLParser.ShowTablesContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#showColumns.
    def visitShowColumns(self, ctx:SQLParser.ShowColumnsContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#createTable.
    def visitCreateTable(self, ctx:SQLParser.CreateTableContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#tableElement.
    def visitTableElement(self, ctx:SQLParser.TableElementContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#columnDef.
    def visitColumnDef(self, ctx:SQLParser.ColumnDefContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#intType.
    def visitIntType(self, ctx:SQLParser.IntTypeContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#floatType.
    def visitFloatType(self, ctx:SQLParser.FloatTypeContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#charType.
    def visitCharType(self, ctx:SQLParser.CharTypeContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#varcharType.
    def visitVarcharType(self, ctx:SQLParser.VarcharTypeContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#dateType.
    def visitDateType(self, ctx:SQLParser.DateTypeContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#boolType.
    def visitBoolType(self, ctx:SQLParser.BoolTypeContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#colPrimaryKey.
    def visitColPrimaryKey(self, ctx:SQLParser.ColPrimaryKeyContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#colNotNull.
    def visitColNotNull(self, ctx:SQLParser.ColNotNullContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#colNull.
    def visitColNull(self, ctx:SQLParser.ColNullContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#colUnique.
    def visitColUnique(self, ctx:SQLParser.ColUniqueContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#colDefault.
    def visitColDefault(self, ctx:SQLParser.ColDefaultContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#colCheck.
    def visitColCheck(self, ctx:SQLParser.ColCheckContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#colReferences.
    def visitColReferences(self, ctx:SQLParser.ColReferencesContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#tcPrimaryKey.
    def visitTcPrimaryKey(self, ctx:SQLParser.TcPrimaryKeyContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#tcUnique.
    def visitTcUnique(self, ctx:SQLParser.TcUniqueContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#tcForeignKey.
    def visitTcForeignKey(self, ctx:SQLParser.TcForeignKeyContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#tcCheck.
    def visitTcCheck(self, ctx:SQLParser.TcCheckContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#columnList.
    def visitColumnList(self, ctx:SQLParser.ColumnListContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#dropTable.
    def visitDropTable(self, ctx:SQLParser.DropTableContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#alterTable.
    def visitAlterTable(self, ctx:SQLParser.AlterTableContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#renameTable.
    def visitRenameTable(self, ctx:SQLParser.RenameTableContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#renameColumn.
    def visitRenameColumn(self, ctx:SQLParser.RenameColumnContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#addColumn.
    def visitAddColumn(self, ctx:SQLParser.AddColumnContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#dropColumn.
    def visitDropColumn(self, ctx:SQLParser.DropColumnContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#addConstraint.
    def visitAddConstraint(self, ctx:SQLParser.AddConstraintContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#dropConstraint.
    def visitDropConstraint(self, ctx:SQLParser.DropConstraintContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#alterColumnType.
    def visitAlterColumnType(self, ctx:SQLParser.AlterColumnTypeContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#createIndex.
    def visitCreateIndex(self, ctx:SQLParser.CreateIndexContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#dropIndex.
    def visitDropIndex(self, ctx:SQLParser.DropIndexContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#insertStmt.
    def visitInsertStmt(self, ctx:SQLParser.InsertStmtContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#valuesRow.
    def visitValuesRow(self, ctx:SQLParser.ValuesRowContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#updateStmt.
    def visitUpdateStmt(self, ctx:SQLParser.UpdateStmtContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#assignment.
    def visitAssignment(self, ctx:SQLParser.AssignmentContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#deleteStmt.
    def visitDeleteStmt(self, ctx:SQLParser.DeleteStmtContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#whereClause.
    def visitWhereClause(self, ctx:SQLParser.WhereClauseContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#selectStmt.
    def visitSelectStmt(self, ctx:SQLParser.SelectStmtContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#selectList.
    def visitSelectList(self, ctx:SQLParser.SelectListContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#tableStarItem.
    def visitTableStarItem(self, ctx:SQLParser.TableStarItemContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#exprItem.
    def visitExprItem(self, ctx:SQLParser.ExprItemContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#tableRef.
    def visitTableRef(self, ctx:SQLParser.TableRefContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#innerJoin.
    def visitInnerJoin(self, ctx:SQLParser.InnerJoinContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#leftJoin.
    def visitLeftJoin(self, ctx:SQLParser.LeftJoinContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#rightJoin.
    def visitRightJoin(self, ctx:SQLParser.RightJoinContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#crossJoin.
    def visitCrossJoin(self, ctx:SQLParser.CrossJoinContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#groupByClause.
    def visitGroupByClause(self, ctx:SQLParser.GroupByClauseContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#havingClause.
    def visitHavingClause(self, ctx:SQLParser.HavingClauseContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#orderByClause.
    def visitOrderByClause(self, ctx:SQLParser.OrderByClauseContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#orderItem.
    def visitOrderItem(self, ctx:SQLParser.OrderItemContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#limitClause.
    def visitLimitClause(self, ctx:SQLParser.LimitClauseContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#predicateExpr.
    def visitPredicateExpr(self, ctx:SQLParser.PredicateExprContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#notExpr.
    def visitNotExpr(self, ctx:SQLParser.NotExprContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#orExpr.
    def visitOrExpr(self, ctx:SQLParser.OrExprContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#andExpr.
    def visitAndExpr(self, ctx:SQLParser.AndExprContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#comparePred.
    def visitComparePred(self, ctx:SQLParser.ComparePredContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#isNullPred.
    def visitIsNullPred(self, ctx:SQLParser.IsNullPredContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#inPred.
    def visitInPred(self, ctx:SQLParser.InPredContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#likePred.
    def visitLikePred(self, ctx:SQLParser.LikePredContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#betweenPred.
    def visitBetweenPred(self, ctx:SQLParser.BetweenPredContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#valuePred.
    def visitValuePred(self, ctx:SQLParser.ValuePredContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#columnExpr.
    def visitColumnExpr(self, ctx:SQLParser.ColumnExprContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#unaryExpr.
    def visitUnaryExpr(self, ctx:SQLParser.UnaryExprContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#funcExpr.
    def visitFuncExpr(self, ctx:SQLParser.FuncExprContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#addExpr.
    def visitAddExpr(self, ctx:SQLParser.AddExprContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#literalExpr.
    def visitLiteralExpr(self, ctx:SQLParser.LiteralExprContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#mulExpr.
    def visitMulExpr(self, ctx:SQLParser.MulExprContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#parenExpr.
    def visitParenExpr(self, ctx:SQLParser.ParenExprContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#functionCall.
    def visitFunctionCall(self, ctx:SQLParser.FunctionCallContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#columnRef.
    def visitColumnRef(self, ctx:SQLParser.ColumnRefContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#intLiteral.
    def visitIntLiteral(self, ctx:SQLParser.IntLiteralContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#floatLiteral.
    def visitFloatLiteral(self, ctx:SQLParser.FloatLiteralContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#stringLiteral.
    def visitStringLiteral(self, ctx:SQLParser.StringLiteralContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#boolLiteral.
    def visitBoolLiteral(self, ctx:SQLParser.BoolLiteralContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#nullLiteral.
    def visitNullLiteral(self, ctx:SQLParser.NullLiteralContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#identifier.
    def visitIdentifier(self, ctx:SQLParser.IdentifierContext):
        return self.visitChildren(ctx)


    # Visit a parse tree produced by SQLParser#nonReserved.
    def visitNonReserved(self, ctx:SQLParser.NonReservedContext):
        return self.visitChildren(ctx)



del SQLParser