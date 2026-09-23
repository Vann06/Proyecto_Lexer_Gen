# IDE-lite — panel semántico

El IDE web del proyecto: editor con resaltado, sidebar de archivos y un solo
botón **▶ ANALIZAR** que corre el pipeline completo (`POST /api/pipeline`)
sobre TODO el archivo de prueba. Es lo que levanta `docker-compose.yml` en
`:4000`.

Modos de parser: **LALR(1)** y **SLR(1)**. No hay LL(1): en ese modo el
backend no corre el análisis semántico (la transformación de la gramática
renombra producciones, ver `api::pipeline`), así que las vistas quedarían
vacías.

El IDE anterior (stepper LR paso a paso, DFA, FIRST/FOLLOW, tablas
ACTION/GOTO, generación de código) se eliminó; sigue en el historial de git
(`frontend/IDE-full/`) si hiciera falta.

## El slot `.g4`

Un cuarto slot de archivo — **`.g4` de referencia** (p.ej. el
`Compiscript.g4` de la raíz del repo) — que se carga y se ve en el editor con
resaltado propio, pero nunca se manda al backend ni se persiste al workspace
(`sanitize_filename` en `src/bin/api.rs` no lo acepta): sirve para comparar
visualmente la gramática ANTLR original contra su traducción en `.yalp`
mientras se prueba. Este generador no compila `.g4`.

## Las cinco vistas

Todas leen directo de la respuesta de `POST /api/pipeline` — nada se
recalcula ni se reconstruye en el cliente:

| Pestaña | Campo de `/api/pipeline` |
|---|---|
| TOKENS | `token_map` |
| ÁRBOL SINTÁCTICO | `parse_tree_dot` — sale auto-anotado con el tipo de cada expresión en cuanto hay análisis semántico (el "árbol de análisis anotado" del libro del dragón: mismo campo, el backend decide en `api/pipeline.rs` si lo dibuja plano o anotado según si corrió `analyze()`) |
| SÍMBOLOS | `symbol_table` (estado final: Global + miembros de funciones/clases) **+** `scopes` (una foto de cada entorno Function/Class/Block al cerrarse — incluye los locales de un bloque anónimo que `symbol_table` no puede mostrar) |
| TIPOS | `types` — el tipo inferido de cada nodo de expresión, en una tabla; el `id` de cada fila coincide con el nodo correspondiente en el árbol |
| PROBLEMAS | `problems` completo: errores léxicos (`L`), sintácticos (`P`) y semánticos (`S`), advertencias (`W001` símbolo sin usar, `W002` código inalcanzable) y avisos del IDE (`E`), ordenados por línea. Clic en uno con posición: el editor salta a esa línea, que además queda marcada con una franja roja (error) o amarilla (advertencia) |

## Cómo levantarlo

```bash
docker compose up --build     # IDE-lite en :4000, API en :8080
```

Sin Docker:

```bash
cargo run --bin api
python3 -m http.server 5500 --directory frontend/IDE-lite
```
