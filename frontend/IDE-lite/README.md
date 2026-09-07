# IDE-lite — panel semántico

Mismo IDE que [`frontend/IDE-full/`](../IDE-full/README.md) — mismo CSS
pixel/retro, mismo editor con resaltado y sidebar de archivos, mismo `D`
global (`data.jsx`) — pero con los paneles que no hacían falta para probar el
análisis semántico **quitados**, no un rediseño desde cero. Es lo que levanta
`docker-compose.yml` en `:4000` por defecto.

## Qué se sacó

- Pestañas GRAMÁTICA, FIRST, FOLLOW, ESTADOS, ACTION/GOTO, LR(0), CÓD.GEN,
  CLOSURES.
- El stepper PARSE CONSOLE (traza paso a paso, ACTION/GOTO resaltado por
  paso) y toda la fila inferior de la grilla que ocupaba.
- El flujo compilar-gramática → auto-parsear-primera-línea del IDE completo:
  acá un solo botón **▶ ANALIZAR** corre el pipeline completo sobre TODO el
  archivo de prueba de una vez (tiene más sentido para un `.cps` completo que
  para casos de una línea).

## Qué se agregó

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
| ERRORES SEM. | `problems` filtrado a los códigos `S0xx` |

Todo esto (`scopes`, `types`, el árbol auto-anotado) ya existe en
`frontend/IDE-full/` — acá solo se aisló en menos pestañas. Nada de esto
necesitó cambios en el backend.

## Cómo levantarlo

```bash
docker compose up --build     # IDE-lite en :4000, API en :8080
```

Sin Docker:

```bash
cargo run --bin api
python3 -m http.server 5500 --directory frontend/IDE-lite
```
