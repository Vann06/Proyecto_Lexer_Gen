# Plan del intérprete SQL

Cómo se llega del estado actual al enunciado: SQL → léxico → sintáctico → AST →
semántico → interpretación → archivos de texto → resultado, **sin ningún DBMS
existente**. Cada fase cierra una o más entregas del curso.

| Fase | Entregas | Contenido | Estado |
|---|---|---|---|
| A | 1–2 | Lexer/parser ANTLR, errores LEX/SYN, AST, subconsultas, vistas de tokens, árbol y AST | ✅ |
| B | 4 (base) | Almacenamiento en archivos, catálogo y tipos | ✅ |
| C | 3 | Análisis semántico (`SEMxxx`) | ✅ |
| D1 | 4 | CREATE/DROP TABLE, INSERT, SELECT simple | ✅ |
| D2 | 5 | Evaluador de expresiones, WHERE, ORDER BY, UPDATE, DELETE | ✅ |
| D3 | 6 | INNER JOIN, agregados, GROUP BY, HAVING | pendiente |
| D4 | 7 | Índices y subconsultas | índices ✅; subconsultas pendientes |
| E | 8 | API de ejecución, IDE (EJECUTAR, RESULTADOS, CATÁLOGO), ejemplos y documentación | pendiente |

B va antes que C porque la semántica valida contra el catálogo. D va de lo
fácil a lo difícil para tener algo funcionando cuanto antes. El reparto entre
las tres personas está al final, en [Reparto del trabajo](#reparto-del-trabajo).

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
- `dbms/storage.py`: `DataDir` (una carpeta por base; `principal` se crea sola; no se llama `default` porque es palabra reservada), `Database` (`load_catalog`/`save_catalog`, `read_rows`/`write_rows`, crear/borrar el .csv), `dump_metadata`/`parse_metadata` con errores `archivo:línea`. Escritura atómica (`.tmp` + `os.replace`).
- `_rowid` único por fila y nunca reutilizado (`NEXT_ROWID`): es la dirección estable a la que apuntarán los índices.
- FOREIGN KEY y CHECK se guardan pero no se hacen cumplir (fuera de alcance).
- Pruebas: `tests/test_sqltypes.py`, `tests/test_catalog.py`, `tests/test_storage.py`.

## C. Análisis semántico — ✅

`dbms/semantic.py`: recorre el AST contra el catálogo **antes** de ejecutar y acumula `Problem("err", "SEMxxx", msg, line, col)` sin detenerse en el primero.

- `check(stmt, catalog)`: una sentencia (la usará la ejecución, sentencia por sentencia).
- `check_script(stmts, catalog, data)`: un script entero sin ejecutar (botón ANALIZAR). Trabaja sobre una copia del catálogo y le aplica el efecto de cada sentencia correcta (CREATE/DROP TABLE e INDEX, CREATE/DROP DATABASE, USE): un INSERT ve la tabla creada antes en el mismo script.
- Conectado a `POST /api/sql/parse` (contra `data/<database>`, por defecto `principal`) y al IDE: los SEM salen en PROBLEMAS y en la barra de estado.

| Código | Error |
|---|---|
| SEM001 | Tabla inexistente (también la referenciada por una FOREIGN KEY) |
| SEM002 | Tabla ya existe (CREATE sin IF NOT EXISTS) |
| SEM003 | Columna inexistente (SELECT, WHERE, SET, listas de columnas, restricciones) |
| SEM004 | Referencia ambigua: columna en varias tablas del FROM/JOIN, o tabla/alias repetido |
| SEM005 | Calificador inválido (`x.col` o `x.*` con `x` que no está en el FROM) |
| SEM006 | Tipos incompatibles: comparación, aritmética, `\|\|`, LIKE, condición no booleana, fecha literal inválida, FOREIGN KEY entre tipos distintos |
| SEM007 | INSERT: número de valores distinto al de columnas |
| SEM008 | INSERT/UPDATE: valor incompatible con el tipo de la columna (incluye VARCHAR demasiado largo, fecha inválida) |
| SEM009 | NULL en columna NOT NULL (literal, o columna NOT NULL omitida en el INSERT) |
| SEM010 | Índice inexistente (DROP INDEX) |
| SEM011 | Índice duplicado (CREATE INDEX) |
| SEM012 | Subconsulta escalar o de `IN` con más de una columna |
| SEM013 | Agregado mal usado: en WHERE/ON/GROUP BY/SET, anidado, o columna fuera de GROUP BY |
| SEM014 | Función desconocida (solo COUNT, SUM, AVG, MIN, MAX) o aridad incorrecta |
| SEM015 | Base de datos inexistente (USE, DROP DATABASE) |
| SEM016 | Base de datos ya existe |
| SEM017 | Nombre repetido en una lista (columnas de CREATE TABLE, INSERT, SET, índice, restricción) |
| SEM018 | Definición de tabla inválida (dos PRIMARY KEY, VARCHAR(0), FOREIGN KEY a columnas que no son PK/UNIQUE o con distinta cantidad) |
| SEM099 | No soportado todavía: ALTER TABLE, ALTER DATABASE, LEFT/RIGHT/CROSS JOIN (error); DEFAULT (advertencia: se ignora) |

Reglas de diseño:
- **Ámbitos**: cada SELECT abre uno con sus tablas, colgado del de la consulta externa. Una columna sin calificar se busca primero adentro y después afuera: así funcionan las subconsultas correlacionadas.
- **Sin cascada**: si una tabla no existe, sus columnas se aceptan en silencio (un solo SEM001, no uno por columna).
- **Fechas**: no hay literal DATE en la gramática; una cadena se acepta contra una columna DATE si tiene formato `YYYY-MM-DD` válido.
- **GROUP BY estricto** (como el estándar): toda columna del SELECT/HAVING/ORDER BY fuera de un agregado debe estar en el GROUP BY. `ORDER BY` puede usar los alias del SELECT.
- "Más de un registro" en una escalar solo se sabe al ejecutar: va como `EXE`.
- La base por defecto se llama `principal` (no `default`: es palabra reservada y `USE default` no parsearía).

Pruebas: `tests/test_semantic.py` (un caso que pasa y uno que falla por cada código, posiciones, correlación, GROUP BY, scripts, USE). `workspace/errores_semanticos.sql` tiene un error por línea, marcado con su código; un test verifica que cada línea dé exactamente ese código.

## D. Ejecución

Cada sentencia devuelve un `Result` (columnas + filas, o un mensaje "n filas
afectadas", más el `plan` usado). El ejecutor confía en que la semántica ya
validó tablas, columnas y tipos: solo reporta errores que dependen de los
datos (`EXE…`: llave duplicada, escalar con varios registros, …).

Archivos, para que cada persona trabaje en los suyos sin pisarse:

| Archivo | Qué hace | Sub-fase |
|---|---|---|
| `dbms/result.py` | `Result` (contrato con la API y el IDE) | D1 |
| `dbms/executor.py` | `execute(stmt, db) -> Result`: despacha por tipo de sentencia | D1 |
| `dbms/eval.py` | `evaluate(expr, env)`: valor de una expresión para una fila | D2 |
| `dbms/select.py` | tubería del SELECT: FROM → WHERE → GROUP → HAVING → ORDER → proyección | D1–D2 |
| `dbms/join.py` | INNER JOIN | D3 |
| `dbms/aggregate.py` | agregados, GROUP BY, HAVING | D3 |
| `dbms/index.py` | archivos `.idx`, búsqueda, mantenimiento | D4 |
| `dbms/subquery.py` | ejecución de subconsultas con el contexto de la fila externa | D4 |

### D1. CREATE, DROP, INSERT y SELECT simple (entrega 4) — ✅

- `CREATE TABLE` (con `IF NOT EXISTS`): `table_from_ast` → catálogo → `.csv` vacío.
- `DROP TABLE` (con `IF EXISTS`): borra el `.csv`, sus índices y la entrada del catálogo.
- `INSERT`: columnas omitidas = NULL, `coerce` de cada valor, `_rowid` = `NEXT_ROWID` (y se incrementa).
  Hace cumplir NOT NULL, PRIMARY KEY y UNIQUE (`EXE001` llave duplicada, `EXE002` NULL en columna NOT NULL).
- `SELECT * / col, col FROM t` sin WHERE: lectura y proyección.
- Definir `Result` aquí: lo usan D2–D4 y la fase E.
- **Hecho cuando**: crear, insertar y consultar de punta a punta en `tmp_path`, revisando también los archivos.

### D2. Evaluador, WHERE, ORDER BY, UPDATE y DELETE (entrega 5) — ✅

- `eval.py`: literales, columnas (`col` y `t.col`), aritmética, `||`, comparaciones, AND/OR/NOT, IS NULL, IN (lista).
  **Lógica de tres valores**: `NULL = 1` es desconocido; WHERE solo deja pasar verdadero.
- El `env` de evaluación es una pila de ámbitos (fila actual + filas externas): la misma pieza la usan D3 (varias tablas) y D4 (subconsultas correlacionadas).
- `WHERE` en SELECT, UPDATE y DELETE; `UPDATE … SET col = expr` con `coerce`.
- `ORDER BY` con varias claves, ASC/DESC, por alias o por expresión; NULL al final.
- **Hecho cuando**: `SELECT nombre FROM estudiantes WHERE edad >= 18;` (el ejemplo del enunciado) y UPDATE/DELETE revisando el `.csv`.

### D3. INNER JOIN, agregados, GROUP BY y HAVING (entrega 6) — 🔴 pesada

- INNER JOIN por bucles anidados; nombres calificados y alias (`e.id`, `i.estudiante_id`).
- COUNT/SUM/AVG/MIN/MAX: `COUNT(*)` vs `COUNT(col)` (no cuenta NULL), `DISTINCT`, NULL ignorado en el resto.
- GROUP BY una o varias columnas; sin GROUP BY y con agregados = un solo grupo.
- HAVING y ORDER BY pueden usar agregados (`HAVING COUNT(*) > 1`, `ORDER BY SUM(nota) DESC`).
- **Hecho cuando**: consultas de estudiantes × inscripciones × cursos con conteos y promedios por curso.

### D4. Índices y subconsultas (entrega 7) — 🔴 pesada

**Índices**
Implementados y probados por Persona 2. Formato preciso, acceso directo al CSV,
contratos de extensiones y límites: [docs/motor.md](docs/motor.md).

- `CREATE INDEX` construye `<idx>.idx` (`valor|rowid,rowid,…` ordenado); `DROP INDEX` lo borra.
- En WHERE, una conjunción `col = literal` o un rango (`<`, `<=`, `>`, `>=`) sobre una columna indexada se resuelve con búsqueda binaria en el índice y solo se leen esas filas. `Result.plan` dice `"índice idx_edad"` o `"recorrido completo"` para poder demostrarlo.
- INSERT, UPDATE y DELETE mantienen todos los índices de la tabla.

**Subconsultas**
- IN / NOT IN (cuidado: `x NOT IN (…)` con un NULL en la subconsulta nunca es verdadero).
- EXISTS / NOT EXISTS correlacionados: se reevalúan por cada fila externa; las no correlacionadas se calculan una vez.
- Escalar: NULL si no devuelve filas; `EXE003` si devuelve más de una.
- **Hecho cuando**: los ejemplos de subconsultas de `workspace/demo.sql` dan el resultado correcto y una consulta con índice muestra el plan.

- **Toda la fase D**: tests de punta a punta por sentencia sobre una base en `tmp_path`, comparando filas y archivos resultantes.

## E. Integración

- API: `POST /api/sql/run {script, database}` → por sentencia: problemas, resultado, plan; `GET /api/catalog`.
- IDE: botón **▶ EJECUTAR** (además de ANALIZAR), pestañas **RESULTADOS** (tabla por sentencia) y **CATÁLOGO** (tablas, columnas, tipos, índices); errores SEM/EXE en PROBLEMAS.
- `workspace/`: scripts de demostración por entrega (estudiantes, cursos, inscripciones).
- README y guía de uso actualizados.

---

## Fuera de alcance (ya parsean; se ejecutan solo si sobra tiempo)

Dos grupos, según lo que hace hoy la semántica:

- **Se validan normal y son baratos de ejecutar** (conviene agregarlos en D2/D3 si hay tiempo): LIKE, BETWEEN, DISTINCT, LIMIT/OFFSET, CREATE/DROP DATABASE, USE, SHOW TABLES/DATABASES, DESCRIBE.
- **La semántica los marca `SEM099`**: ALTER TABLE, ALTER DATABASE, LEFT/RIGHT/CROSS JOIN (error) y DEFAULT (advertencia; se ignora).
- FOREIGN KEY y CHECK se validan al crear la tabla y se guardan, pero no se hacen cumplir al insertar.

---

## Reparto del trabajo

Tres personas, cada una con una parte inicial que no depende de las demás y
una segunda parte que arranca cuando la base de otra ya está lista. Cambiar
"Persona 1/2/3" por los nombres.

| | Primera parte | Segunda parte | Archivos que le pertenecen |
|---|---|---|---|
| **Persona 1** · semántica | **C** completa | **D4 · subconsultas** | `semantic.py`, `subquery.py`, `tests/test_semantic.py`, `tests/test_subquery.py` |
| **Persona 2** · motor | **D1 + D2** | **D4 · índices** | `result.py`, `executor.py`, `eval.py`, `select.py`, `index.py` y sus tests |
| **Persona 3** · integración | **E** (API + IDE) | **D3** | `api/main.py`, `frontend/IDE-lite/`, `workspace/`, `join.py`, `aggregate.py` y sus tests |

Por qué así:
- **Carga parecida.** Cada persona tiene una parte media y una pesada: C + subconsultas, D1–D2 + índices, E + D3.
- **Afinidad.** Las subconsultas correlacionadas son sobre todo resolver nombres entre ámbitos, que es lo mismo que hace la semántica. Los índices se meten en el WHERE y en INSERT/UPDATE/DELETE, que son del motor. JOIN y GROUP BY son etapas del SELECT que se enchufan a `select.py` sin tocar el evaluador.
- **Nadie espera al arrancar.** C solo necesita el catálogo (B, listo). D1 también. E puede armar el endpoint y las pestañas con D1 apenas exista, sin esperar a lo demás.

### Contratos a acordar el primer día

Son las piezas que una persona produce y otra consume. Si se fijan primero, cada quien avanza por su lado.

| Contrato | Lo define | Lo usan | Forma propuesta |
|---|---|---|---|
| Semántica | Persona 1 | Persona 3 (API) | `check(stmt, catalog) -> list[Problem]` |
| `Result` | Persona 2 | Persona 3 (API, IDE), todas en tests | `columns: list[str]`, `rows: list[list]`, `message: str`, `plan: str \| None` |
| Entorno de evaluación | Persona 2 | Personas 1 y 3 | `evaluate(expr, env)`; `env` = pila de ámbitos `{nombre_o_alias: fila}` con acceso a las filas externas; un gancho `env.run_subquery(select)` que implementa Persona 1 |
| Etapas del SELECT | Persona 2 | Persona 3 | `select.py` llama a `join.rows(...)` y `aggregate.group(...)`; Persona 3 implementa esas funciones |
| `/api/sql/run` | Persona 3 | IDE | por sentencia: `{kind, sql, line, problems, result}`; se detiene en la primera sentencia con error |

### Orden en el tiempo

| Etapa | Persona 1 | Persona 2 | Persona 3 |
|---|---|---|---|
| 1 | C: resolución de tablas/columnas, ambigüedad, tipos, INSERT (`SEM001`–`SEM009`) | D1: `Result`, CREATE/DROP, INSERT con PK/UNIQUE/NOT NULL, SELECT simple | E: `/api/sql/run` (parse → semántica → ejecución), `/api/catalog`, botón EJECUTAR, pestañas RESULTADOS y CATÁLOGO |
| 2 | C: índices, subconsultas (`SEM012`), agregados (`SEM013`), funciones (`SEM014`) | D2: evaluador con NULL, WHERE, ORDER BY, UPDATE, DELETE | D3: INNER JOIN (cuando el evaluador básico de D2 ya esté en `SQL`) |
| 3 | D4: IN / EXISTS / escalar, correlación | D4: `.idx`, uso en WHERE, mantenimiento, `plan` | D3: agregados, GROUP BY, HAVING |
| 4 | Todas: scripts de demostración por entrega (lidera Persona 3), README, corrección de errores y, si sobra tiempo, lo de "Fuera de alcance" | | |

Dependencias a vigilar:
- **D3 necesita el evaluador de D2.** Persona 2 debe integrar pronto una primera versión de `eval.py` (columnas, literales, comparaciones, AND/OR), aunque falten detalles.
- **Las subconsultas necesitan D2 y D3.** `(SELECT AVG(edad) FROM estudiantes)` usa agregados. Por eso van en la etapa 3.
- **E se integra de a poco.** El endpoint funciona desde D1 y cada fase nueva se ve en el IDE sin cambiarlo.

### Forma de trabajo con git

- Una rama por persona desde `SQL`: `sql/semantica`, `sql/motor`, `sql/integracion`.
- PRs pequeños hacia `SQL`, uno por sub-fase o parte de ella, con sus tests. `python -m pytest` debe pasar antes de integrar.
- Cada quien edita sus archivos. Para tocar un archivo de otra persona (por ejemplo, agregar un caso al despacho de `executor.py`), se avisa o se hace en su PR.
- `PLAN.md` y la tabla de estado del README se actualizan al terminar cada sub-fase.
