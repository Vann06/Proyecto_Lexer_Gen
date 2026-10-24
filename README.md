# Intérprete SQL con almacenamiento en archivos (ANTLR)

Intérprete de **SQL** con mecanismo de almacenamiento propio: bases de datos
como directorios y tablas, esquemas e índices como archivos de texto
(`metadata.txt`, `.csv`, `.idx`). No usa SQLite, MySQL, PostgreSQL ni ningún
otro DBMS. El análisis léxico y sintáctico lo genera **ANTLR 4** desde
[`backend/grammar/SQL.g4`](backend/grammar/SQL.g4); el resto (AST, semántica,
ejecución, almacenamiento) está en Python. Incluye una API HTTP y un IDE web
(`frontend/IDE-lite/`).

> La versión anterior del proyecto (generador propio de lexers/parsers en
> Rust + Compiscript) sigue en la rama `main`.

## Estado

El detalle de cada fase (formatos de archivo, códigos de error, criterio de
"hecho") y el reparto del trabajo entre el equipo están en [`PLAN.md`](PLAN.md).

| Fase | Entregas | Contenido | Estado |
|---|---|---|---|
| A | 1–2 | Lexer/parser, errores LEX/SYN, AST, subconsultas (IN, EXISTS, escalares), vistas de tokens, árbol y AST | ✅ |
| B | 4 (base) | Almacenamiento `metadata.txt` + `.csv`, catálogo y tipos | ✅ |
| C | 3 | Análisis semántico (`SEMxxx`) | ✅ |
| D1 | 4 | CREATE/DROP TABLE, INSERT, SELECT simple | ✅ |
| D2 | 5 | Evaluador de expresiones, WHERE, ORDER BY, UPDATE, DELETE | ✅ |
| D3 | 6 | INNER JOIN, agregados, GROUP BY, HAVING | pendiente |
| D4 | 7 | Índices y subconsultas | pendiente |
| E | 8 | API `/api/sql/run`, IDE (EJECUTAR, RESULTADOS, CATÁLOGO), ejemplos y documentación | pendiente |

Hoy el botón **▶ ANALIZAR** del IDE hace el análisis léxico, sintáctico y
semántico (contra la base `data/principal/`): muestra tokens, el árbol de
derivación de ANTLR, el AST y los errores con línea y columna. Todavía no
ejecuta sentencias desde la interfaz. El motor D1/D2 está disponible mediante
`dbms.executor.execute`; la integración del botón EJECUTAR pertenece a E.

## Estructura

```text
backend/
├── grammar/SQL.g4        # fuente de verdad del lexer y el parser
├── dbms/
│   ├── parser/           # GENERADO por ANTLR (versionado; no editar)
│   ├── frontend/         # parse(): errores → Problem, tokens, árbol → DOT
│   ├── ast.py            # nodos del AST (dataclasses)
│   ├── builder.py        # visitor de ANTLR → AST
│   ├── ast_dot.py        # AST → DOT (pestaña AST del IDE)
│   ├── sqltypes.py       # tipos: validación, compatibilidad, texto del CSV
│   ├── catalog.py        # esquema en memoria: tablas, restricciones, índices
│   ├── storage.py        # data/<db>/metadata.txt + <tabla>.csv
│   └── semantic.py       # análisis semántico contra el catálogo (SEMxxx)
├── api/main.py           # FastAPI
└── tests/                # pytest
frontend/IDE-lite/        # IDE React (nginx en Docker)
workspace/                # scripts .sql que ve el IDE (demo.sql, errores_sintaxis.sql, errores_semanticos.sql)
data/                     # bases de datos (no versionado; DBMS_DATA para cambiarlo)
scripts/gen_parser.*      # regenera backend/dbms/parser/ desde SQL.g4
```

## Uso

**Con Docker:** `docker compose up --build` → IDE en http://localhost:4000,
API en http://localhost:8080.

**Local:**

```sh
cd backend
pip install -r requirements-dev.txt
python -m pytest                              # pruebas
python -m uvicorn api.main:app --port 8080    # API
# en otra terminal, servir el IDE:
cd frontend/IDE-lite && python -m http.server 4000
```

**Regenerar el parser** después de cambiar `SQL.g4` (la primera vez
`antlr4-tools` ofrece descargar Java; hay que aceptar):

```sh
sh scripts/gen_parser.sh        # o: powershell scripts/gen_parser.ps1
```

El CI falla si `backend/dbms/parser/` no coincide con la gramática.

## Códigos de error

| Prefijo | Fase | Ejemplo |
|---|---|---|
| `LEX` | léxico | `LEX001` carácter no reconocido, `LEX002` cadena sin cerrar |
| `SYN` | sintáctico | `SYN001` entrada inesperada, `SYN002` entrada sobrante, `SYN003` falta un token, `SYN004` sentencia no válida |
| `SEM` | semántico | `SEM001` tabla inexistente, `SEM003` columna inexistente, `SEM004` referencia ambigua, `SEM006` tipos incompatibles… (tabla completa en [`PLAN.md`](PLAN.md#c-análisis-semántico--)) |
| `EXE` | ejecución | (fase D) |
