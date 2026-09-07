# IDE completo (archivado)

Este es el IDE original: editor con resaltado, stepper de parseo paso a paso
(PARSE CONSOLE), tabla ACTION/GOTO, colección canónica de estados LR,
autómata LR(0) en Graphviz, FIRST/FOLLOW, generación de código del lexer, y
las vistas de símbolos/closures/tipos que fueron sumando las fases
semánticas.

Se archivó tal cual (sin cambios) al crear
[`frontend/IDE-lite/`](../IDE-lite/), una versión reducida pensada para
probar rápido el análisis semántico con vistas más limpias — ver el README
de esa carpeta. `docker-compose.yml` ahora levanta `IDE-lite` por defecto en
`:4000`; este IDE completo sigue siendo el de referencia para todo lo demás
(stepper, autómatas, codegen), así que no se tocó nada de su código.

## Cómo levantarlo (sin Docker)

```bash
cargo run --bin api                                    # API en :8080
python3 -m http.server 5500 --directory frontend/IDE-full   # UI en :5500
```

Abrir `http://localhost:5500/IDE%20Analizador%20Sintactico.html` — la raíz
del sitio (`/`) sirve un listado de directorio en vez del IDE porque nginx
es quien renombra ese archivo a `index.html` al construir la imagen Docker
(ver `Dockerfile`); `http.server` no lo hace.

## Cómo levantarlo con Docker (aparte de docker-compose.yml)

```bash
docker build -t syntra-ide-full ./frontend/IDE-full
docker run --rm -p 4001:80 syntra-ide-full
```

(usa `:4001` para no chocar con `IDE-lite` si ambos están corriendo a la vez
— la API sigue siendo la misma en `:8080`, `app.jsx` la tiene hardcodeada en
la línea 5).

## Archivos

- `IDE Analizador Sintactico.html` — shell HTML, carga React/Babel/viz.js por
  CDN y `data.jsx`/`app.jsx` con `<script type="text/babel">`.
- `data.jsx` — `window.IDE_DATA`, el estado mutable que todos los componentes
  de `app.jsx` leen (alias `D`).
- `app.jsx` — un componente por panel: `Editor`, `StatesView`,
  `ActionGotoTable`, `LR0Graph`, `ParseTreeView`, `SymbolTableView`,
  `TypesView`, `ClosuresView`, `ProblemsList`, `ParseConsole` (el stepper),
  etc.
- `Dockerfile` — nginx sirviendo esta carpeta, renombrando el `.html` a
  `index.html`.
