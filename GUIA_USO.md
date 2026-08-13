# Guía de uso — SYNTRA IDE

## Levantar el sistema

### Con Docker (recomendado)

```bash
# Primera vez — construye las imágenes (≈ 3–5 min por compilación Rust)
docker compose up --build

# Siguientes veces — usa caché
docker compose up

# Apagar
docker compose down
```

| Servicio | URL |
|----------|-----|
| **IDE (frontend)** | [http://localhost:4000](http://localhost:4000) |
| API Rust | [http://localhost:8080](http://localhost:8080) |
| Health check | [http://localhost:8080/health](http://localhost:8080/health) |

---

### Sin Docker (desarrollo local)

```bash
# Terminal 1 — servidor Rust
cargo run --bin api

# Terminal 2 — servidor frontend en http://localhost:5500
python3 -m http.server 5500 --directory frontend/IDE
```

Luego abrir [http://localhost:5500](http://localhost:5500) en el navegador.

---

## Flujo de trabajo en el IDE

### 1. Cargar archivos

En el panel izquierdo bajo **CARGAR ARCHIVOS**, usa los botones para subir:

- `↑ .yal / .yalex` — definiciones léxicas
- `↑ .yalp / .yapar` — gramática del parser
- `↑ input.txt` — cadena de prueba

Los archivos se guardan automáticamente en la carpeta `workspace/` del proyecto.  
Al abrir el IDE, los archivos del workspace se cargan solos.

---

### 2. Editar y guardar

El editor es completamente editable con syntax highlighting en tiempo real.  
Haz clic en **SAVE** (esquina superior derecha) para escribir los cambios a disco.

---

### 3. Compilar — botón RUN

Haz clic en **▶ RUN**.

El botón muestra `...` mientras el backend procesa. Al terminar, los paneles de resultados se actualizan:

| Tab         | Qué muestra                                       |
|-------------|---------------------------------------------------|
| GRAMÁTICA   | Producciones numeradas + terminales/no-terminales |
| FIRST       | Conjuntos FIRST de cada no-terminal               |
| FOLLOW      | Conjuntos FOLLOW de cada no-terminal              |
| ESTADOS     | Colección canónica con ítems y lookaheads         |
| ACTION/GOTO | Tabla completa con la celda activa resaltada      |
| LR(0)       | Autómata LR(0) como grafo interactivo             |
| PROBLEMAS   | Conflictos S/R o R/R, o confirmación sin errores  |

Modos disponibles (selector en el header): **LALR(1)**, **SLR(1)**, **LL(1)**

---

### 4. Parsear una cadena

1. Escribe los tokens en el campo inferior (separados por espacios), ej: `c c d c d`
2. Presiona **▶ PARSEAR** o `Enter`
3. Navega la traza paso a paso:

| Botón  | Acción                        |
|--------|-------------------------------|
| ⏮      | Primer paso                   |
| ◀ PASO | Retroceder un paso            |
| PASO ▶ | Avanzar un paso               |
| ⏭      | Último paso (resultado final) |

---

## API — endpoints

### Health

[GET http://localhost:8080/health](http://localhost:8080/health)

```json
{ "status": "ok", "service": "syntra-api" }
```

---

### Workspace

[GET http://localhost:8080/api/workspace](http://localhost:8080/api/workspace) — lista de archivos

```json
{ "files": [ { "name": "lexer.yal", "kind": "yal" }, ... ] }
```

`GET http://localhost:8080/api/workspace/:nombre` — leer archivo

`PUT http://localhost:8080/api/workspace/:nombre` — guardar archivo (body = texto plano)

---

### Compilar gramática

`POST http://localhost:8080/api/parser/compile`

```json
// Request
{ "content": "%token c d\n%%\nS : C C ;\nC : c C | d ;\n", "mode": "lalr" }

// Response
{
  "states":        [ { "id": 0, "items": ["S' → • S , $", ...] } ],
  "action":        { "0": { "c": "s3", "d": "s4" } },
  "goto":          { "0": { "S": 1, "C": 2 } },
  "terminals":     ["c", "d", "$"],
  "non_terminals": ["S", "C"],
  "first":         { "S": ["c","d"], "C": ["c","d"] },
  "follow":        { "S": ["$"], "C": ["$","c","d"] },
  "prods":         [ { "n": 1, "lhs": "S", "rhs": ["C","C"] } ],
  "problems":      [ { "level": "info", "code": "I100", "msg": "sin conflictos" } ],
  "lr0_dot":       "digraph { ... }"
}
```

Valores de `mode`: `lalr` (default) · `slr` · `ll1`

---

### Parsear tokens

`POST http://localhost:8080/api/parser/parse`

```json
// Request
{ "content": "...", "tokens": ["c", "c", "d", "c", "d"], "mode": "lalr" }

// Response
{
  "trace": [
    { "stack": [0, "c", 3], "remaining": ["c","d","c","d","$"], "action": "s3", "desc": "Shift 'c' → I3" }
  ],
  "accepted": true,
  "error": null
}
```

`trace[].stack` **no** es una lista de estados: es un array intercalado
`[estado, símbolo, estado, símbolo, …]` — siempre un estado más que símbolos,
así que el último elemento es siempre el estado actual (`stack[stack.length-1]`
en el frontend). En modo `ll1` la forma del paso es distinta (`stack`/`remaining`
son pilas de símbolos, no estados, y cada paso trae `pos`).

---

### Pipeline completo (.yal + .yalp + fuente → traza)

`POST http://localhost:8080/api/pipeline`

Combina lexer + parser en una sola llamada: tokeniza `source` con el `.yal`
dado y parsea el resultado con el `.yalp` dado. Pensado para UN caso de
prueba por llamada (una línea/expresión) — no agrupa ni separa por saltos de
línea internamente, así que `source` puede ser un programa de varias líneas.

```json
// Request
{
  "yal_content":  "let digit = ['0'-'9']\nrule tokens =\n  | digit+ { return NUM }\n",
  "yalp_content": "%token NUM\n%%\nS : NUM ;\n",
  "source": "42",
  "mode": "lalr"
}

// Response — mismo shape que /api/parser/parse, más:
{
  "trace": [ /* igual que arriba, cada paso con "line" además */ ],
  "accepted": true,
  "error": null,
  "problems":  [ /* errores léxicos (L001) y sintácticos (P001), con line/col */ ],
  "token_map": [ { "kind": "NUM", "lexeme": "42", "line": 1, "col": 1 } ]
}
```

En modo `lalr`/`slr`, si hay más de un error de sintaxis independiente,
`problems` los reporta TODOS (recuperación en modo pánico) — no solo el
primero.

---

## Formato del archivo .yalp

```
%token TOKEN_A TOKEN_B TOKEN_C
%start S

%%

S : TOKEN_A B TOKEN_C ;

B : TOKEN_B B
  | TOKEN_A
  ;
```

- `%token` declara terminales
- `%start` declara el símbolo inicial (opcional, por defecto la primera producción)
- `%%` separa cabecera de producciones
- Producciones con `:` y terminadas en `;`
- Alternativas con `|`

---

## Cadenas válidas para la gramática de ejemplo

Gramática: `S → C C`, `C → c C | d`

| Cadena        | Válida |
|---------------|--------|
| `d d`         | ✓      |
| `c d c d`     | ✓      |
| `c c d c c d` | ✓      |
| `c d`         | ✗      |
| `d c d`       | ✗      |
