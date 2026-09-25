# DBMS con SQL (ANTLR)

Sistema gestor de bases de datos que interpreta **SQL**: creación de bases y
tablas con restricciones, DML, consultas con JOIN/GROUP BY e índices. El
análisis léxico y sintáctico lo genera **ANTLR 4** desde
[`backend/grammar/SQL.g4`](backend/grammar/SQL.g4); el resto (AST, semántica,
ejecución, almacenamiento en JSON) está en Python. Incluye una API HTTP y un
IDE web (`frontend/IDE-lite/`).

> La versión anterior del proyecto (generador propio de lexers/parsers en
> Rust + Compiscript) sigue en la rama `main`.

## Estado

| Fase | Contenido | Estado |
|---|---|---|
| 0 | Limpieza y andamiaje (Python, ANTLR, Docker, CI) | ✅ |
| 1 | Gramática `SQL.g4` | ✅ |
| 2 | Parse → errores en español → AST (`dbms.ast`) | ✅ |
| 3 | Tipos, catálogo y almacenamiento JSON | pendiente |
| 4 | Análisis semántico contra el catálogo | pendiente |
| 5 | Ejecución DDL + restricciones (PK, FK, UNIQUE, CHECK, NOT NULL) | pendiente |
| 6 | INSERT / UPDATE / DELETE / SELECT básico | pendiente |
| 7 | JOIN, GROUP BY + agregados, índices | pendiente |
| 8 | API `/api/sql/run`, `/api/catalog` | pendiente |
| 9 | IDE: pestañas RESULTADOS y CATÁLOGO | pendiente |
| 10 | Pruebas finales, ejemplos y documentación | pendiente |

Hoy el botón **▶ ANALIZAR** del IDE hace el análisis léxico y sintáctico:
muestra tokens, el árbol de derivación de ANTLR y los errores con línea y
columna. Todavía no ejecuta sentencias.

## Estructura

```text
backend/
├── grammar/SQL.g4        # fuente de verdad del lexer y el parser
├── dbms/
│   ├── parser/           # GENERADO por ANTLR (versionado; no editar)
│   ├── frontend/         # parse(): errores → Problem, tokens, árbol → DOT
│   ├── ast.py            # nodos del AST (dataclasses)
│   └── builder.py        # visitor de ANTLR → AST
├── api/main.py           # FastAPI
└── tests/                # pytest
frontend/IDE-lite/        # IDE React (nginx en Docker)
workspace/                # scripts .sql que ve el IDE (demo.sql, errores_sintaxis.sql)
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
| `SEM` | semántico | (fase 4) |
| `EXE` | ejecución | (fases 5–7) |
