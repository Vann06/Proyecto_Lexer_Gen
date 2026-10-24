# Persona 2: motor SQL

Implementación de D1, D2 y la parte de índices de D4 en la rama SQL.
El enunciado exige interpretar SQL con archivos propios, sin un DBMS externo;
esta implementación usa el AST y almacenamiento del proyecto, sin SQLite ni
otro motor. JOIN, agregados, ejecución de subconsultas y API/IDE siguen siendo
entregas de las otras personas. No se declara completa la entrega 7 ni la 8.

## Requisitos y evidencia

| Área | Implementación | Pruebas |
|---|---|---|
| D1: CREATE/DROP TABLE, IF EXISTS/IF NOT EXISTS | catálogo, CSV, índices implícitos PK/UNIQUE y limpieza | `test_executor.py` |
| D1: INSERT | coerción, columnas omitidas NULL, PK/UNIQUE/NOT NULL, rowid creciente | `test_executor.py` |
| D1/D2: SELECT | proyección, alias, WHERE, ORDER BY múltiple, NULL al final | `test_select.py` |
| D2: expresiones | aritmética, concatenación, comparaciones, AND/OR/NOT, IS NULL, IN | `test_eval.py` |
| D2: UPDATE/DELETE | WHERE, asignaciones simultáneas, restricciones antes de escritura | `test_executor.py` |
| D4: CREATE/DROP INDEX | índices persistidos, ordenados por valor tipado, búsqueda binaria | `test_index.py` |
| D4: acceso por índice | igualdad/rangos y conjunciones, lectura por posiciones del CSV | pruebas que prohíben `Database.read_rows` en consultas indexadas |
| D4: mantenimiento | reconstrucción de todos los índices afectados y posiciones | comparación con recorrido completo después de mutaciones y reapertura |

También se incluyen LIKE, BETWEEN, DISTINCT, LIMIT/OFFSET, SHOW TABLES y
DESCRIBE/SHOW COLUMNS. Se prueban fechas, booleanos, floats, claves compuestas,
valores duplicados y nulos, texto Unicode, separadores y celdas multilínea.

## Uso desde Python

Desde `backend`, con las dependencias de `requirements-dev.txt` instaladas:

```python
from pathlib import Path
from dbms.builder import build
from dbms.frontend import parse
from dbms.semantic import check
from dbms.executor import execute
from dbms.storage import DataDir

db = DataDir(Path("data")).open(create=True)
parsed = parse("""
CREATE TABLE estudiantes (id INT PRIMARY KEY, nombre VARCHAR(30), edad INT);
INSERT INTO estudiantes VALUES (1, 'Ana', 19), (2, 'Luis', 17);
CREATE INDEX idx_edad ON estudiantes (edad);
SELECT nombre FROM estudiantes WHERE edad >= 18;
""")
assert parsed.ok, parsed.problems
for stmt in build(parsed):
    problems = check(stmt, db.load_catalog())
    assert not any(p.level == "err" for p in problems), problems
    print(execute(stmt, db).to_dict())
```

El último resultado contiene `columns=['nombre']`, `rows=[['Ana']]` y
`plan='índice idx_edad'`. La semántica se ejecuta sentencia por sentencia sobre
el catálogo actualizado. `execute` recibe `Statement` o su cuerpo y presupone
que ya fue validado; no sustituye `semantic.check`.

## Contratos para las otras personas

- `Result(columns, rows, message, plan)`: valores DATE en memoria son `date`;
  `to_dict()` los convierte a ISO para JSON. Los errores se lanzan como
  `ExecutionError`; `to_problem()` produce el formato de la API con ubicación.
- `Environment.scopes`: lista del ámbito local al externo; cada ámbito es
  `{alias_o_tabla: {columna: valor}}`. `push(scope)` crea un entorno hijo y
  conserva los callbacks. Un alias local oculta el alias externo.
- Persona 1 instala `Environment(subquery_runner=callback)`; el callback
  recibe `(select_ast, environment_actual)` y devuelve `Result`. El evaluador
  maneja IN/NOT IN, EXISTS y cardinalidad escalar, pero no ejecuta ni cachea
  subconsultas por sí solo. La Persona 1 implementa ese recorrido y la caché.
- Persona 3 implementa `dbms.join.rows(stmt, db, envs)` y
  `dbms.aggregate.group(stmt, envs)`, que devuelven entornos. `join.rows`
  combina las tablas en el mismo ámbito local y aplica ON. `aggregate.group`
  instala `aggregate_runner(func_call, env)` en cada entorno de grupo y debe
  producir el grupo vacío cuando corresponda a un agregado global.
  Los módulos se cargan al necesitarse; también pueden inyectarse mediante
  `execute(..., join_rows=..., group=...)`.
- Orden de etapas: FROM/JOIN → WHERE → GROUP → HAVING → proyección/DISTINCT →
  claves de ORDER BY (incluyendo alias) → orden → LIMIT/OFFSET. La proyección
  conserva el entorno original para ordenar por columnas no proyectadas.
- Mientras una extensión no esté conectada se devuelve un error EXE099;
  nunca se ignora silenciosamente JOIN, GROUP BY o una función agregada.

## Formato y funcionamiento de índices

`<nombre>.idx` contiene una línea por clave: `clave_JSON|rowid,rowid,...`.
Las claves simples son escalares; las compuestas son listas JSON. DATE usa
ISO, NULL usa `null`; las barras verticales dentro de cadenas se escapan como
`\u007c`. Se ordena por valores tipados, no por el texto serializado; NULL queda
al final. Ejemplo: `8|1,3` representa dos filas con valor numérico 8.

`<tabla>.csv.pos` es un directorio JSON de rowid a `[offset, longitud]` en bytes
UTF-8. Se deriva leyendo registros CSV completos, incluyendo campos con saltos
de línea. SELECT busca con `bisect` en el índice y usa `seek` para leer solo los
registros seleccionados. Se cargan el índice y el directorio en memoria, pero
no se recorre ni se materializa toda la tabla para resolver ese SELECT.

Se optimizan `=`, `<`, `<=`, `>` y `>=` sobre una columna, incluso con el literal
a la izquierda, alias y números negativos. En AND se intersectan los índices
aplicables y después se evalúa el WHERE completo sobre los candidatos. OR y
expresiones no indexables usan recorrido completo para preservar resultados.
Los índices compuestos se guardan y mantienen, pero no optimizan búsquedas
por prefijo en esta entrega.

Cada mutación valida primero todos los valores y restricciones. INSERT/UPDATE/
DELETE reescriben el CSV y reconstruyen los índices y posiciones. UPDATE y
DELETE usan los índices para elegir candidatos, pero requieren leer y escribir
el archivo completo para conservar el resto de las filas. Este costo de
reescritura es una limitación del almacenamiento CSV, no una consulta SELECT.

UNIQUE permite varias claves con NULL; una PRIMARY KEY no. Borrar un índice
implícito no elimina la restricción: esta se verifica también desde el catálogo.
Los rowids borrados no se reutilizan. DROP TABLE elimina CSV, posiciones,
índices y catálogo asociado.

## Errores y límites

| Código | Significado |
|---|---|
| EXE001 | PK, UNIQUE o índice único duplicado |
| EXE002 | NULL en columna NOT NULL |
| EXE003 | subconsulta escalar con más de una fila (callback) |
| EXE004 | error aritmético o valor incompatible en ejecución |
| EXE005 | columna no disponible o ambigua en el entorno |
| EXE006 | subconsulta con cantidad de columnas inválida (callback) |
| EXE099 | sentencia o extensión aún no implementada |

La división INT/INT trunca hacia cero para respetar el tipo que infiere la
semántica existente; con FLOAT devuelve FLOAT. WHERE acepta únicamente TRUE.
Las operaciones con NULL usan lógica de tres valores, incluso IN/NOT IN.

Errores de datos no dejan cambios parciales: se prueba la igualdad de todos los
archivos antes y después del fallo, incluyendo NEXT_ROWID e índices. Cada
archivo se reemplaza atómicamente según `storage.py`; **no hay transacción
atómica entre varios archivos**, recuperación ante apagados ni bloqueo para
escritores concurrentes. No se deben editar CSV/índices a mano. FOREIGN KEY y
CHECK permanecen almacenados pero no se aplican, según el alcance de PLAN.md.

## Pruebas

Verificación del 24 de octubre de 2026: **302 pruebas aprobadas** (73 nuevas
del motor y 229 existentes), sin fallos. La suite emite una advertencia de
deprecación de Starlette sobre su integración con httpx; no afecta el resultado.

```sh
cd backend
python -m pytest -q
```

En el entorno local preparado para esta tarea, desde `backend`:

```powershell
../tmp/motor-venv/Scripts/python.exe -m pytest -q
```

Las pruebas usan bases temporales y no modifican `data/`. La comparación
aleatoria es reproducible (semilla fija); incluye 25 predicados por estado de
la tabla, antes y después de UPDATE/DELETE. Los tests de extensiones usan
callbacks mínimos para comprobar los contratos sin implementar D3/D4 ajenos.
