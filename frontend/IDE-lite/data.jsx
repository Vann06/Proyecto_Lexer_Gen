/* ============================================================================
   IDE-lite: el `D` global mutable que comparten todas las vistas — los
   archivos abiertos y lo que devolvió la última llamada a /api/sql/parse.

   Dos slots:
   - sql: el script que se analiza. Al arrancar se carga del workspace
     (workspace/demo.sql si existe); SQL_FALLBACK solo se ve si la API no
     responde.
   - g4:  la gramática ANTLR real (backend/grammar/SQL.g4, vía GET
     /api/grammar). Solo lectura: es la fuente de verdad del lexer y el
     parser, se muestra para poder consultarla mientras se escribe SQL.
   ========================================================================== */

const SQL_FALLBACK = "-- La API no respondió (¿está corriendo en :8080?).\n-- Mientras tanto se puede escribir SQL aquí.\n\nCREATE DATABASE tienda;\nUSE tienda;\n\nCREATE TABLE cliente (\n    id     INT PRIMARY KEY,\n    nombre VARCHAR(50) NOT NULL\n);\n\nINSERT INTO cliente VALUES (1, 'Ana'), (2, 'Luis');\n\nSELECT * FROM cliente WHERE id > 0 ORDER BY nombre;\n";

const FILES = {
  sql: { name: "demo.sql", kind: "sql", rawContent: SQL_FALLBACK, dirty: false },
  g4:  { name: "SQL.g4",   kind: "g4",  rawContent: "// cargando backend/grammar/SQL.g4 desde la API…\n", dirty: false, readonly: true },
};

window.IDE_DATA = {
  FILES,
  // [{i, k, lx, l, c}] — campo `tokens` de /api/sql/parse
  TOKENS: [],
  // [{level, code, msg, loc, line, col}] — LEX/SYN del backend, IDE del cliente
  PROBLEMS: [],
  // null = todavía no se analizó; true/false = sin/con errores de sintaxis
  PARSE_OK: null,
  // Árbol de derivación de ANTLR en DOT — campo `parse_tree_dot`
  PARSE_TREE_DOT: "",
  // [{kind, sql, line, col}] — sentencias reconocidas (vacío si hubo errores)
  STATEMENTS: [],
};
