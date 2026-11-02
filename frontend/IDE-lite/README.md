# IDE-lite

El IDE web del intérprete SQL: editor con resaltado, panel de archivos y las
vistas de cada etapa del compilador. Para usarlo, ver la
[guía de uso](../../docs/guia_uso.md#3-recorrido-por-el-ide).

## Cómo está hecho

React 18 sin paso de compilación: `index.html` carga React, Babel standalone y
[viz.js](https://github.com/mdaines/viz-js) desde un CDN, y transpila en el
navegador.

| Archivo | Contenido |
|---|---|
| `index.html` | Estilos (tema retro) y carga de los scripts |
| `data.jsx` | `window.IDE_DATA`: el estado compartido (archivos abiertos, última respuesta de la API) |
| `app.jsx` | Componentes: editor, explorador, pestañas de resultados, barra de estado |
| `Dockerfile` | nginx sirviendo los archivos estáticos (lo usa `docker-compose.yml`) |

La URL de la API está en la constante `API` al inicio de `app.jsx`
(`http://localhost:8080`).

## De dónde sale cada pestaña

Ninguna vista recalcula nada en el cliente: todas muestran campos de la API.

| Pestaña | Endpoint | Campo |
|---|---|---|
| TOKENS | `POST /api/sql/parse` | `tokens` |
| ÁRBOL SINTÁCTICO | `POST /api/sql/parse` | `parse_tree_dot` (DOT → SVG con viz.js) |
| AST | `POST /api/sql/parse` | `ast_dot` |
| PROBLEMAS | `/api/sql/parse` o `/api/sql/run` | `problems` (`LEX`, `SYN`, `SEM`, `EXE`, e `IDE001` si la API no responde) |
| RESULTADOS | `POST /api/sql/run` | `statements[].result` y `statements[].problems` |
| CATÁLOGO | `GET /api/catalog?database=…` | `tables`, `indexes` |

El panel WORKSPACE usa `GET /api/workspace`, `GET`/`PUT /api/workspace/{nombre}`;
la pestaña `SQL.g4` usa `GET /api/grammar`.

## Levantarlo

```bash
docker compose up --build                                   # con la API, desde la raíz
python -m http.server 4000 --directory frontend/IDE-lite    # solo el IDE (la API aparte)
```
