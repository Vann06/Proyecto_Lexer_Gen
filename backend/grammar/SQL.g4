// Gramática SQL del DBMS. Fuente única de verdad del análisis léxico y
// sintáctico: el lexer y el parser en backend/dbms/parser/ se generan desde
// aquí con scripts/gen_parser.ps1 (o .sh). No editar los archivos generados.
//
// Convenciones:
// - Las palabras clave no distinguen mayúsculas (caseInsensitive).
// - Las funciones (COUNT, SUM, UPPER, ...) NO son palabras clave: son
//   identificadores seguidos de '('. Qué funciones existen lo decide la
//   fase semántica, no la gramática.
// - Toda alternativa que el constructor de AST distingue lleva etiqueta
//   (#nombre) para que el visitor tenga un método por caso.

grammar SQL;

options { caseInsensitive = true; }

// ───────────────────────────── Script ─────────────────────────────

script
    : sqlStmt? (SEMI sqlStmt?)* EOF
    ;

sqlStmt
    : createDatabase
    | dropDatabase
    | alterDatabase
    | useDatabase
    | showDatabases
    | showTables
    | showColumns
    | createTable
    | dropTable
    | alterTable
    | createIndex
    | dropIndex
    | insertStmt
    | updateStmt
    | deleteStmt
    | selectStmt
    ;

// ───────────────────────────── Bases de datos ─────────────────────────────

createDatabase : CREATE DATABASE (IF NOT EXISTS)? identifier ;
dropDatabase   : DROP DATABASE (IF EXISTS)? identifier ;
alterDatabase  : ALTER DATABASE old=identifier RENAME TO new=identifier ;
useDatabase    : USE DATABASE? identifier ;
showDatabases  : SHOW DATABASES ;
showTables     : SHOW TABLES ;
showColumns
    : SHOW COLUMNS FROM identifier
    | (DESCRIBE | DESC) identifier
    ;

// ───────────────────────────── Tablas ─────────────────────────────

createTable
    : CREATE TABLE (IF NOT EXISTS)? identifier
      LPAREN tableElement (COMMA tableElement)* RPAREN
    ;

tableElement
    : columnDef
    | tableConstraint
    ;

columnDef : identifier dataType columnConstraint* ;

dataType
    : (INT | INTEGER)                        # intType
    | (FLOAT | REAL | DOUBLE)                # floatType
    | CHAR LPAREN INT_LIT RPAREN             # charType
    | VARCHAR LPAREN INT_LIT RPAREN          # varcharType
    | DATE                                   # dateType
    | (BOOLEAN | BOOL)                       # boolType
    ;

columnConstraint
    : (CONSTRAINT cname=identifier)? PRIMARY KEY                                  # colPrimaryKey
    | NOT NULL                                                                    # colNotNull
    | NULL                                                                        # colNull
    | (CONSTRAINT cname=identifier)? UNIQUE                                       # colUnique
    | DEFAULT valueExpr                                                           # colDefault
    | (CONSTRAINT cname=identifier)? CHECK LPAREN expr RPAREN                     # colCheck
    | (CONSTRAINT cname=identifier)? REFERENCES ref=identifier
      (LPAREN refCol=identifier RPAREN)?                                          # colReferences
    ;

tableConstraint
    : (CONSTRAINT cname=identifier)? PRIMARY KEY columnList                       # tcPrimaryKey
    | (CONSTRAINT cname=identifier)? UNIQUE columnList                            # tcUnique
    | (CONSTRAINT cname=identifier)? FOREIGN KEY cols=columnList
      REFERENCES ref=identifier refCols=columnList?                               # tcForeignKey
    | (CONSTRAINT cname=identifier)? CHECK LPAREN expr RPAREN                     # tcCheck
    ;

columnList : LPAREN identifier (COMMA identifier)* RPAREN ;

dropTable : DROP TABLE (IF EXISTS)? identifier ;

alterTable : ALTER TABLE identifier alterAction (COMMA alterAction)* ;

alterAction
    : RENAME TO identifier                                         # renameTable
    | RENAME COLUMN old=identifier TO new=identifier               # renameColumn
    | ADD COLUMN? columnDef                                        # addColumn
    | DROP COLUMN identifier                                       # dropColumn
    | ADD tableConstraint                                          # addConstraint
    | DROP CONSTRAINT identifier                                   # dropConstraint
    | ALTER COLUMN identifier (SET DATA)? TYPE dataType            # alterColumnType
    ;

// ───────────────────────────── Índices ─────────────────────────────

createIndex
    : CREATE UNIQUE? INDEX (IF NOT EXISTS)? idx=identifier ON tbl=identifier columnList
    ;

dropIndex : DROP INDEX (IF EXISTS)? identifier ;

// ───────────────────────────── DML ─────────────────────────────

insertStmt
    : INSERT INTO identifier columnList? VALUES valuesRow (COMMA valuesRow)*
    ;

valuesRow : LPAREN expr (COMMA expr)* RPAREN ;

updateStmt : UPDATE identifier SET assignment (COMMA assignment)* whereClause? ;

assignment : identifier EQ expr ;

deleteStmt : DELETE FROM identifier whereClause? ;

whereClause : WHERE expr ;

// ───────────────────────────── SELECT ─────────────────────────────

selectStmt
    : SELECT DISTINCT? selectList
      (FROM tableRef joinClause*)?
      whereClause?
      groupByClause?
      havingClause?
      orderByClause?
      limitClause?
    ;

selectList
    : STAR
    | selectItem (COMMA selectItem)*
    ;

selectItem
    : identifier DOT STAR                  # tableStarItem
    | expr (AS? alias=identifier)?         # exprItem
    ;

tableRef : tbl=identifier (AS? alias=identifier)? ;

joinClause
    : (INNER)? JOIN tableRef ON expr       # innerJoin
    | LEFT OUTER? JOIN tableRef ON expr    # leftJoin
    | RIGHT OUTER? JOIN tableRef ON expr   # rightJoin
    | CROSS JOIN tableRef                  # crossJoin
    ;

groupByClause : GROUP BY expr (COMMA expr)* ;
havingClause  : HAVING expr ;
orderByClause : ORDER BY orderItem (COMMA orderItem)* ;
orderItem     : expr (ASC | DESC)? ;
limitClause   : LIMIT limit=INT_LIT (OFFSET offset=INT_LIT)? ;

// ───────────────────────────── Expresiones ─────────────────────────────
//
// Tres niveles para que la precedencia no dependa del orden de alternativas
// de una sola regla (y para que el AND de BETWEEN no se confunda con el AND
// lógico):
//   expr      — lógica: NOT > AND > OR
//   predicate — comparaciones, IS NULL, IN, LIKE, BETWEEN
//   valueExpr — aritmética: unario > * / % > + - ||

expr
    : NOT expr                 # notExpr
    | expr AND expr            # andExpr
    | expr OR expr             # orExpr
    | predicate                # predicateExpr
    ;

predicate
    : valueExpr op=(EQ | NEQ | LT | LTE | GT | GTE) valueExpr                       # comparePred
    | valueExpr IS NOT? NULL                                                         # isNullPred
    | valueExpr NOT? IN LPAREN valueExpr (COMMA valueExpr)* RPAREN                   # inPred
    | valueExpr NOT? LIKE valueExpr                                                  # likePred
    | valueExpr NOT? BETWEEN low=valueExpr AND high=valueExpr                        # betweenPred
    | valueExpr                                                                      # valuePred
    ;

valueExpr
    : op=(MINUS | PLUS) valueExpr                                # unaryExpr
    | valueExpr op=(STAR | SLASH | PERCENT) valueExpr            # mulExpr
    | valueExpr op=(PLUS | MINUS | CONCAT) valueExpr             # addExpr
    | LPAREN expr RPAREN                                         # parenExpr
    | functionCall                                               # funcExpr
    | columnRef                                                  # columnExpr
    | literal                                                    # literalExpr
    ;

functionCall
    : fname=identifier LPAREN (STAR | DISTINCT? expr (COMMA expr)*)? RPAREN
    ;

columnRef : (tbl=identifier DOT)? col=identifier ;

literal
    : INT_LIT                  # intLiteral
    | FLOAT_LIT                # floatLiteral
    | STRING_LIT               # stringLiteral
    | (TRUE | FALSE)           # boolLiteral
    | NULL                     # nullLiteral
    ;

// Palabras clave que también se aceptan como nombres (columnas llamadas
// "date" o "type" son demasiado comunes para prohibirlas).
identifier
    : IDENTIFIER
    | QUOTED_IDENTIFIER
    | nonReserved
    ;

nonReserved : DATA | DATE | TYPE | COLUMNS | TABLES | DATABASES ;

// ───────────────────────────── Lexer ─────────────────────────────

ADD        : 'ADD';
ALTER      : 'ALTER';
AND        : 'AND';
AS         : 'AS';
ASC        : 'ASC';
BETWEEN    : 'BETWEEN';
BOOL       : 'BOOL';
BOOLEAN    : 'BOOLEAN';
BY         : 'BY';
CHAR       : 'CHAR';
CHECK      : 'CHECK';
COLUMN     : 'COLUMN';
COLUMNS    : 'COLUMNS';
CONSTRAINT : 'CONSTRAINT';
CREATE     : 'CREATE';
CROSS      : 'CROSS';
DATA       : 'DATA';
DATABASE   : 'DATABASE';
DATABASES  : 'DATABASES';
DATE       : 'DATE';
DEFAULT    : 'DEFAULT';
DELETE     : 'DELETE';
DESC       : 'DESC';
DESCRIBE   : 'DESCRIBE';
DISTINCT   : 'DISTINCT';
DOUBLE     : 'DOUBLE';
DROP       : 'DROP';
EXISTS     : 'EXISTS';
FALSE      : 'FALSE';
FLOAT      : 'FLOAT';
FOREIGN    : 'FOREIGN';
FROM       : 'FROM';
GROUP      : 'GROUP';
HAVING     : 'HAVING';
IF         : 'IF';
IN         : 'IN';
INDEX      : 'INDEX';
INNER      : 'INNER';
INSERT     : 'INSERT';
INT        : 'INT';
INTEGER    : 'INTEGER';
INTO       : 'INTO';
IS         : 'IS';
JOIN       : 'JOIN';
KEY        : 'KEY';
LEFT       : 'LEFT';
LIKE       : 'LIKE';
LIMIT      : 'LIMIT';
NOT        : 'NOT';
NULL       : 'NULL';
OFFSET     : 'OFFSET';
ON         : 'ON';
OR         : 'OR';
ORDER      : 'ORDER';
OUTER      : 'OUTER';
PRIMARY    : 'PRIMARY';
REAL       : 'REAL';
REFERENCES : 'REFERENCES';
RENAME     : 'RENAME';
RIGHT      : 'RIGHT';
SELECT     : 'SELECT';
SET        : 'SET';
SHOW       : 'SHOW';
TABLE      : 'TABLE';
TABLES     : 'TABLES';
TO         : 'TO';
TRUE       : 'TRUE';
TYPE       : 'TYPE';
UNIQUE     : 'UNIQUE';
UPDATE     : 'UPDATE';
USE        : 'USE';
VALUES     : 'VALUES';
VARCHAR    : 'VARCHAR';
WHERE      : 'WHERE';

EQ      : '=';
NEQ     : '<>' | '!=';
LTE     : '<=';
GTE     : '>=';
LT      : '<';
GT      : '>';
PLUS    : '+';
MINUS   : '-';
STAR    : '*';
SLASH   : '/';
PERCENT : '%';
CONCAT  : '||';
LPAREN  : '(';
RPAREN  : ')';
COMMA   : ',';
SEMI    : ';';
DOT     : '.';

FLOAT_LIT
    : DIGIT+ '.' DIGIT* EXPONENT?
    | '.' DIGIT+ EXPONENT?
    | DIGIT+ EXPONENT
    ;
INT_LIT : DIGIT+ ;

// Comilla simple escapada duplicándola: 'O''Brien'
STRING_LIT : '\'' ( ~'\'' | '\'\'' )* '\'' ;

// Una cadena sin cerrar llega hasta el final del archivo; se reconoce como
// token propio para dar un error claro en vez de "token recognition error".
UNTERMINATED_STRING : '\'' ( ~'\'' | '\'\'' )* EOF ;

IDENTIFIER        : [a-z_] [a-z_0-9]* ;
QUOTED_IDENTIFIER : '"' ( ~'"' | '""' )+ '"' | '`' ~'`'+ '`' ;

LINE_COMMENT  : '--' ~[\r\n]* -> skip ;
BLOCK_COMMENT : '/*' .*? '*/' -> skip ;
WS            : [ \t\r\n]+ -> skip ;

fragment DIGIT    : [0-9] ;
fragment EXPONENT : 'E' [+-]? DIGIT+ ;
