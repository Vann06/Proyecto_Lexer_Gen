# Guía de uso

Cómo instalar, levantar y usar el intérprete SQL: el IDE, los scripts de
demostración, la API y qué hacer cuando algo no funciona.

← [Volver al README](../README.md)

## Contenido

1. [Requisitos](#1-requisitos)
2. [Levantar el proyecto](#2-levantar-el-proyecto)
3. [Recorrido por el IDE](#3-recorrido-por-el-ide)
4. [Presentar las entregas](#4-presentar-las-entregas)
5. [Escribir SQL propio](#5-escribir-sql-propio)
6. [Bases de datos y archivos](#6-bases-de-datos-y-archivos)
7. [Usar la API directamente](#7-usar-la-api-directamente)
8. [Correr las pruebas](#8-correr-las-pruebas)
9. [Problemas frecuentes](#9-problemas-frecuentes)

---

## 1. Requisitos

| Para | Necesitas |
|---|---|
| Usarlo con Docker | Docker Desktop (o Docker Engine + Compose) |
| Usarlo sin Docker | Python **3.11 o superior** y un navegador |
| Cambiar la gramática | Lo anterior; `antlr4-tools` descarga Java la primera vez |

No hace falta instalar ninguna base de datos: el intérprete guarda todo en archivos de texto.

## 2. Levantar el proyecto

### Opción A: Docker

```bash
docker compose up --build
```

| Servicio | Dirección |
|---|---|
| IDE | <http://localhost:4000> |
| API | <http://localhost:8080> |
| Documentación interactiva de la API | <http://localhost:8080/docs> |

Las bases quedan en `./data/` del repositorio (montado en el contenedor), así que
sobreviven a `docker compose down` y `up`. Para apagar: `Ctrl+C` o `docker compose down`.

### Opción B: sin Docker

**Terminal 1, la API:**

```bash
cd backend
pip install -r requirements-dev.txt
python -m uvicorn api.main:app --port 8080
```

**Terminal 2, el IDE** (desde la raíz del repositorio):

```bash
python -m http.server 4000 --directory frontend/IDE-lite
```

Abrir <http://localhost:4000>. El IDE se comunica con la API en `http://localhost:8080`.

> [!TIP]
> Variables de entorno de la API: `DBMS_DATA` cambia la carpeta de las bases
> (por defecto `data/` en la raíz) y `DBMS_WORKSPACE` la de los scripts del
> panel WORKSPACE (por defecto `workspace/`).

## 3. Recorrido por el IDE

```text
┌──────────────┬──────────────────────────┬─────────────────────────────┐
│ EXPLORER     │ Editor SQL               │ RESULTS                     │
│  Cargar      │  (resaltado, números de  │  TOKENS · ÁRBOL SINTÁCTICO  │
│  Abiertos    │   línea, líneas con      │  AST · PROBLEMAS            │
│  WORKSPACE   │   error marcadas)        │  RESULTADOS · CATÁLOGO      │
└──────────────┴──────────────────────────┴─────────────────────────────┘
  ▶ ANALIZAR   ▶ EJECUTAR   SAVE                     barra de estado
```

**Panel izquierdo**
- **CARGAR ARCHIVO**: abre un `.sql` de tu computadora.
- **WORKSPACE**: los scripts de [`workspace/`](../workspace/). Un clic los abre en el editor.
- **SQL.g4**: la gramática ANTLR real, de solo lectura, para consultarla mientras se escribe.

**Botones**

| Botón | Qué hace | Toca los archivos |
|---|---|---|
| **▶ ANALIZAR** | Léxico + sintáctico + semántico de todo el script, contra la base activa | No |
| **▶ EJECUTAR** | Ejecuta sentencia por sentencia; **se detiene en la primera con error** (las anteriores quedan hechas) | Sí |
| **SAVE** | Guarda el script en `workspace/` | Solo el script |

**Pestañas**

| Pestaña | Para qué |
|---|---|
| TOKENS | Ver cómo el lexer cortó el texto: tipo, lexema, línea y columna de cada token |
| ÁRBOL SINTÁCTICO | El árbol de derivación de ANTLR; con errores se dibuja el árbol parcial. Botón **↓ PNG** para descargarlo |
| AST | El árbol limpio que usan la semántica y el motor (solo si no hay errores de sintaxis) |
| PROBLEMAS | Todos los errores con código y fase (léxico, sintáctico, semántico, ejecución). **Clic en uno → el editor salta a esa línea** |
| RESULTADOS | Una tarjeta por sentencia ejecutada: la tabla de resultados o "n filas afectadas", y el **plan** (`índice idx_edad` / `recorrido completo`) |
| CATÁLOGO | Las tablas de la base activa: columnas, tipos, NOT NULL, PK/FK/UNIQUE/CHECK, cantidad de filas, e índices |

La **barra de estado** resume el último análisis: `✓ SINTAXIS OK · n sentencias · ✓ SEMÁNTICA OK`, o la cantidad de errores.

## 4. Presentar las entregas

Hay un script por entrega en [`workspace/`](../workspace/). Los de ejemplo
empiezan con `DROP DATABASE IF EXISTS` + `CREATE DATABASE`, así que **se
pueden ejecutar en cualquier orden y repetir** sin errores de "la tabla ya existe".

| Entrega | Script | Pasos | Qué mostrar |
|---|---|---|---|
| 1 | `entrega1_lexer_parser.sql` | ▶ ANALIZAR | TOKENS (comentarios descartados, `select` = `SELECT`, `'O''Brien'`), ÁRBOL SINTÁCTICO |
| 1 | `entrega1_errores.sql` | ▶ ANALIZAR | PROBLEMAS: un error léxico o sintáctico por línea, con su columna |
| 2 | `entrega2_ast.sql` | ▶ ANALIZAR | AST: precedencia de operadores, `-5` como literal, `NOT EXISTS` como un solo nodo |
| 3 | `entrega3_errores_semanticos.sql` | ▶ ANALIZAR | PROBLEMAS: un error semántico por línea (`SEM001`…`SEM099`) |
| 4 | `entrega4_create_drop_insert.sql` | ▶ EJECUTAR | RESULTADOS y CATÁLOGO: los 5 tipos, restricciones, NULL, fecha bisiesta |
| 5 | `entrega5_select_update_delete.sql` | ▶ EJECUTAR | El ejemplo del enunciado, lógica con NULL, ORDER BY, UPDATE y DELETE |
| 6 | `entrega6_join_agregados.sql` | ▶ EJECUTAR | JOIN de tres tablas, COUNT/SUM/AVG/MIN/MAX, GROUP BY, HAVING |
| 7 | `entrega7_indices_subconsultas.sql` | ▶ EJECUTAR | El **plan** cambia de `recorrido completo` a `índice idx_edad`; subconsultas |
| 8 | `entrega8_integracion.sql` | ▶ ANALIZAR, luego ▶ EJECUTAR | Un sistema de notas completo de punta a punta |

> [!NOTE]
> `entrega3_errores_semanticos.sql` está hecho para **ANALIZAR**: muestra todos
> los errores a la vez. Con EJECUTAR se detiene en el primero, como cualquier
> script con errores.

## 5. Escribir SQL propio

Se puede escribir directo en el editor. Lo más práctico:

```sql
CREATE DATABASE prueba;
USE prueba;
CREATE TABLE estudiantes (id INT PRIMARY KEY, nombre VARCHAR(30) NOT NULL, edad INT);
INSERT INTO estudiantes VALUES (1, 'Ana', 20), (2, 'Luis', 17);
SELECT nombre FROM estudiantes WHERE edad >= 18;
```

- Las sentencias se separan con `;`.
- Las palabras clave no distinguen mayúsculas; los nombres sin comillas se pasan a minúsculas (`"Mi Tabla"` conserva mayúsculas y espacios).
- Las fechas se escriben como cadena: `'2024-02-29'`. Se valida que la fecha exista.
- Comentarios: `-- hasta el fin de línea` y `/* de bloque */`.
- `USE` cambia la base activa; el IDE la recuerda para los siguientes ANALIZAR y EJECUTAR (aparece en RESULTADOS y CATÁLOGO).

La lista completa de lo soportado está en el [README](../README.md#sql-soportado).

## 6. Bases de datos y archivos

```text
data/
├── principal/       ← la base por defecto (se crea sola)
├── universidad/
│   ├── metadata.txt
│   ├── estudiantes.csv
│   ├── estudiantes.csv.pos
│   └── idx_edad.idx
└── …
```

- Cada base es una carpeta. La base por defecto se llama **`principal`** (no `default`, porque `DEFAULT` es palabra reservada de SQL).
- Los archivos se pueden abrir para mirarlos, pero **no conviene editarlos a mano**: el `.idx` y el `.csv.pos` tienen que coincidir con el `.csv`.
- Para empezar de cero una base: `DROP DATABASE nombre;` desde el IDE, o borrar su carpeta con la API apagada.

## 7. Usar la API directamente

La documentación interactiva (Swagger) está en <http://localhost:8080/docs>.

| Método | Ruta | Para qué |
|---|---|---|
| `POST` | `/api/sql/parse` | Analizar sin ejecutar: tokens, árbol, AST, problemas |
| `POST` | `/api/sql/run` | Ejecutar un script; resultado por sentencia |
| `GET` | `/api/catalog?database=…` | Esquema e índices de una base |
| `GET` | `/api/workspace` · `/api/workspace/{nombre}` | Listar y leer los scripts del workspace |
| `PUT` | `/api/workspace/{nombre}` | Guardar un script |
| `GET` | `/api/grammar` | El texto de `SQL.g4` |
| `GET` | `/health` | Comprobar que la API está viva |

Ejemplo:

```bash
curl -s http://localhost:8080/api/sql/run -H "Content-Type: application/json" -d "{\"script\": \"SELECT 1 + 2 AS suma;\"}"
```

Respuesta (resumida):

```json
{
  "ok": true,
  "database": "principal",
  "problems": [],
  "statements": [
    {"kind": "Select", "sql": "SELECT 1 + 2 AS suma", "line": 1, "col": 1, "problems": [],
     "result": {"columns": ["suma"], "rows": [[3]], "message": "", "plan": "recorrido completo"}}
  ]
}
```

Los dos endpoints de SQL aceptan `"database": "nombre"` para elegir la base.

## 8. Correr las pruebas

```bash
cd backend
python -m pytest -q                         # toda la suite
python -m pytest tests/test_rubrica.py -v   # la rúbrica: un test por requisito del enunciado
python -m pytest tests/test_workspace.py    # que los scripts de demostración funcionen
```

Si un test de la rúbrica falla, su nombre (`test_R20_where_usa_el_indice…`)
dice qué requisito dejó de cumplirse, y el mensaje dice qué se esperaba y qué
salió. Las pruebas usan carpetas temporales: nunca tocan `data/`.

## 9. Problemas frecuentes

<details>
<summary><b>PROBLEMAS muestra <code>IDE001 No se pudo contactar la API</code></b></summary>

La API no está corriendo o no está en el puerto 8080. Levantarla (sección 2) y
comprobar <http://localhost:8080/health>.
</details>

<details>
<summary><b>Al volver a ejecutar un script: <code>SEM002 la tabla … ya existe</code></b></summary>

Es correcto: las tablas quedaron creadas en la base activa. Los scripts de
`workspace/` ya empiezan con `DROP DATABASE IF EXISTS`; para un script propio,
agregar `DROP TABLE IF EXISTS …` al inicio o usar otra base.
</details>

<details>
<summary><b>ANALIZAR da errores que antes no daba</b></summary>

ANALIZAR revisa contra la **base activa** (la del último `USE`), que puede tener
otras tablas. Mirar qué base dice RESULTADOS o CATÁLOGO, o empezar el script
con `USE nombre;`.
</details>

<details>
<summary><b>El puerto 8080 o 4000 está ocupado</b></summary>

Otro programa lo usa. Cerrarlo, o usar otro puerto con `--port` / el número de
`http.server`. Si se cambia el de la API, hay que cambiar también la constante
`API` al inicio de `frontend/IDE-lite/app.jsx`.
</details>

<details>
<summary><b><code>git pull</code> falla con "some local refs could not be updated"</b></summary>

En Windows y macOS, una rama remota que empiece con `sql/` choca con la rama
`SQL` (el sistema de archivos no distingue mayúsculas). Las ramas de trabajo se
llaman `SQL-…`, nunca `sql/…`. Ver [PLAN.md](../PLAN.md#forma-de-trabajo-con-git).
</details>

<details>
<summary><b>El CI falla en "Parser generado al día"</b></summary>

Alguien cambió `backend/grammar/SQL.g4` sin regenerar el parser. Correr
`sh scripts/gen_parser.sh` (o `powershell scripts/gen_parser.ps1`) y subir los
archivos de `backend/dbms/parser/`.
</details>
