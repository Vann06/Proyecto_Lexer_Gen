# Plan del intérprete SQL

Cómo se llega del estado actual al enunciado: SQL → léxico → sintáctico → AST →
semántico → interpretación → archivos de texto → resultado, **sin ningún DBMS
existente**. Cada fase cierra una o más entregas del curso.

| Fase | Entregas | Contenido | Estado |
|---|---|---|---|
| A | 1–2 | Lexer/parser ANTLR, errores LEX/SYN, AST, subconsultas, vistas de tokens, árbol y AST | ✅ |
| B | 4 (base) | Almacenamiento en archivos, catálogo y tipos | ✅ |
| C | 3 | Análisis semántico (`SEMxxx`) | pendiente |
| D | 4–7 | Ejecución: DDL, DML, SELECT, JOIN, agregados, índices, subconsultas | pendiente |
| E | 8 | API de ejecución, IDE (EJECUTAR, RESULTADOS, CATÁLOGO), ejemplos y documentación | pendiente |

B va antes que C porque la semántica valida contra el catálogo.

---

## A. Front-end completo — ✅

- `backend/grammar/SQL.g4` → `backend/dbms/parser/` (generado; `scripts/gen_parser.*`).
- `backend/dbms/frontend/parse.py`: errores en español con línea:columna (`LEX001-002`, `SYN001-004`).
- `backend/dbms/ast.py` + `builder.py`: AST con `pos` en cada nodo.
- Subconsultas: `IN / NOT IN (SELECT …)` → `InSubquery`, `EXISTS / NOT EXISTS` → `Exists`
  (el `NOT` se pliega en el builder), `(SELECT …)` como valor → `ScalarSubquery`.
- `backend/dbms/ast_dot.py`: AST → DOT, genérico sobre los dataclasses.
- `POST /api/sql/parse` devuelve `tokens`, `parse_tree_dot`, `ast_dot`, `problems`, `statements`.
- IDE: pestañas TOKENS, ÁRBOL SINTÁCTICO, AST y PROBLEMAS.

## B. Almacenamiento, catálogo y tipos — ✅

Formato (todo texto plano, legible a mano):

```text
data/<db>/
├── metadata.txt        # esquema de todas las tablas e índices
├── estudiantes.csv     # encabezado + filas; primera columna _rowid
└── idx_edad.idx        # (fase D) una línea por valor: "valor|rowid,rowid,..." ordenado por valor
```

`metadata.txt`: todas las restricciones viven aquí, dentro del bloque de su tabla, siempre con nombre.

```text
TABLE estudiantes
  COLUMN id INT NOT NULL
  COLUMN nombre VARCHAR(50) NOT NULL
  COLUMN carne VARCHAR(10)
  PRIMARY KEY pk_estudiantes (id)
  UNIQUE uq_estudiantes_carne (carne)
  NEXT_ROWID 4

TABLE inscripciones
  COLUMN estudiante_id INT NOT NULL
  COLUMN curso_id INT NOT NULL
  COLUMN nota FLOAT
  PRIMARY KEY pk_insc (estudiante_id, curso_id)
  FOREIGN KEY fk_est (estudiante_id) REFERENCES estudiantes (id)
  CHECK ck_inscripciones_1 (nota >= 0)
  NEXT_ROWID 1

INDEX idx_edad ON estudiantes (edad)
INDEX pk_estudiantes ON estudiantes (id) UNIQUE IMPLICIT
```

`estudiantes.csv`:

```text
_rowid,id,nombre,carne
1,1,Ana,A1
2,2,"O'Brien, Pat",\N
```

- `dbms/sqltypes.py`: INT, FLOAT, CHAR(n)/VARCHAR(n), BOOLEAN, DATE (`YYYY-MM-DD`, validada). `coerce` (valida y convierte; INT → FLOAT implícito; `TypeMismatch` con mensaje), `comparable` / `assignable` para la semántica, `to_text` / `from_text` para el CSV (NULL = `\N`).
- `dbms/catalog.py`: `Catalog`, `TableSchema`, `ColumnSchema`, `ConstraintSchema`, `IndexSchema`. `table_from_ast` convierte un `CREATE TABLE`: nombra las restricciones sin nombre (`pk_t`, `uq_t_col`, `fk_t_col`, `ck_t_n`), marca NOT NULL las columnas de la PK y completa `REFERENCES t` con la PK de `t`.
- `dbms/storage.py`: `DataDir` (una carpeta por base; `default` se crea sola), `Database` (`load_catalog`/`save_catalog`, `read_rows`/`write_rows`, crear/borrar el .csv), `dump_metadata`/`parse_metadata` con errores `archivo:línea`. Escritura atómica (`.tmp` + `os.replace`).
- `_rowid` único por fila y nunca reutilizado (`NEXT_ROWID`): es la dirección estable a la que apuntarán los índices.
- FOREIGN KEY y CHECK se guardan pero no se hacen cumplir (fuera de alcance).
- Pruebas: `tests/test_sqltypes.py`, `tests/test_catalog.py`, `tests/test_storage.py`.

## C. Análisis semántico

`dbms/semantic.py`: recorre el AST contra el catálogo **antes** de ejecutar y acumula `Problem("err", "SEMxxx", msg, line, col)` sin detenerse en el primero.

| Código | Error |
|---|---|
| SEM001 | Tabla inexistente |
| SEM002 | Tabla ya existe (CREATE sin IF NOT EXISTS) |
| SEM003 | Columna inexistente |
| SEM004 | Columna ambigua (existe en varias tablas del FROM/JOIN) |
| SEM005 | Calificador inválido (`x.col` con `x` que no está en el FROM) |
| SEM006 | Tipos incompatibles en comparación / aritmética / asignación |
| SEM007 | INSERT: número de valores distinto al de columnas |
| SEM008 | INSERT/UPDATE: valor incompatible con el tipo de la columna (incluye VARCHAR demasiado largo, fecha inválida) |
| SEM009 | NOT NULL violado con un NULL literal |
| SEM010 | Índice inexistente (DROP INDEX) |
| SEM011 | Índice duplicado (CREATE INDEX) |
| SEM012 | Subconsulta escalar o de `IN` con más de una columna |
| SEM013 | Agregado mal usado (en WHERE, o columna fuera de GROUP BY) |
| SEM014 | Función desconocida o con aridad incorrecta |

Las subconsultas correlacionadas resuelven nombres con una pila de ámbitos (la consulta interna ve las tablas de la externa). "Más de un registro" en una escalar solo se sabe al ejecutar: va como `EXE`.

- **Hecho cuando**: un caso que pasa y uno que falla por cada código, con línea y columna.

## D. Ejecución

`dbms/executor.py` (+ `dbms/eval.py` para expresiones). Cada sentencia devuelve un `Result` (columnas + filas, o un mensaje "n filas afectadas").

1. **DDL + INSERT** (entrega 4): CREATE/DROP TABLE, INSERT con PK, NOT NULL y UNIQUE (`EXE001` llave duplicada, …).
2. **SELECT/WHERE/UPDATE/DELETE** (entrega 5): evaluación de expresiones con lógica de tres valores para NULL, AND/OR, ORDER BY (ASC/DESC, varias claves), proyección y alias.
3. **JOIN y agregados** (entrega 6): INNER JOIN por bucles anidados; COUNT/SUM/AVG/MIN/MAX (incluye `COUNT(*)` y `DISTINCT`); GROUP BY y HAVING.
4. **Índices** (entrega 7): CREATE/DROP INDEX construye/borra el `.idx`. En WHERE, si una conjunción es `col = literal` o un rango (`<`, `<=`, `>`, `>=`) sobre una columna indexada, se obtienen los `_rowid` del índice (búsqueda binaria) y solo se leen esas filas; el resultado indica `plan: "índice idx_edad"` vs `"recorrido completo"` para poder demostrarlo. INSERT/UPDATE/DELETE actualizan todos los índices de la tabla.
5. **Subconsultas** (entrega 7): IN/NOT IN, EXISTS/NOT EXISTS (correlacionadas: se reevalúan por fila externa; las no correlacionadas se cachean), escalar (`EXE` si devuelve más de un registro; NULL si ninguno).

- **Hecho cuando**: tests de punta a punta por sentencia sobre una base en `tmp_path`, comparando filas y archivos resultantes.

## E. Integración

- API: `POST /api/sql/run {script, database}` → por sentencia: problemas, resultado, plan; `GET /api/catalog`.
- IDE: botón **▶ EJECUTAR** (además de ANALIZAR), pestañas **RESULTADOS** (tabla por sentencia) y **CATÁLOGO** (tablas, columnas, tipos, índices); errores SEM/EXE en PROBLEMAS.
- `workspace/`: scripts de demostración por entrega (estudiantes, cursos, inscripciones).
- README y guía de uso actualizados.

---

## Fuera de alcance (ya parsean; se ejecutan solo si sobra tiempo)

ALTER TABLE, LEFT/RIGHT/CROSS JOIN, FOREIGN KEY, CHECK, DEFAULT, LIKE, BETWEEN, LIMIT/OFFSET, DISTINCT, CREATE/DROP/ALTER DATABASE, SHOW/DESCRIBE.
Mientras no se implementen, la semántica los reporta como `SEM099 no soportado` en vez de ignorarlos en silencio.
