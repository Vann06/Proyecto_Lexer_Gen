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

| Servicio  | URL                        |
|-----------|----------------------------|
| IDE (UI)  | http://localhost:4000       |
| API Rust  | http://localhost:8080       |

---

### Sin Docker (desarrollo local)

Abre **dos terminales**:

```bash
# Terminal 1 — servidor Rust
cargo run --bin api

# Terminal 2 — abrir el IDE en el navegador
open frontend/IDE/"IDE Analizador Sintactico.html"
# o arrastrar el archivo al navegador
```

---

## Flujo de trabajo en el IDE

### 1. Editar la gramática

En el panel izquierdo haz clic en **parser.yalp**.  
El editor muestra la gramática activa con syntax highlighting.

> Para cambiar la gramática que analiza el IDE, edita `rawContent`  
> de la clave `yalp` en `frontend/IDE/data.jsx`.

---

### 2. Compilar — botón RUN

Haz clic en **▶ RUN** (esquina superior derecha).

El botón muestra `...` mientras el backend procesa. Al terminar, los paneles de resultados se actualizan con datos reales del parser Rust:

| Tab         | Qué muestra                                     |
|-------------|-------------------------------------------------|
| GRAMÁTICA   | Producciones numeradas + terminales/no-terminales |
| FIRST       | Conjuntos FIRST de cada no-terminal              |
| FOLLOW      | Conjuntos FOLLOW de cada no-terminal             |
| ESTADOS     | Colección canónica LR(1) con ítems y lookaheads  |
| ACTION/GOTO | Tabla completa con la celda activa resaltada     |
| PROBLEMAS   | Conflictos S/R o R/R, o confirmación sin errores |

---

### 3. Parsear una cadena — consola inferior

1. Escribe los tokens en el campo de entrada (separados por espacios)  
   Ejemplo: `c c d c d`
2. Presiona **▶ PARSEAR** o `Enter`
3. La traza se carga en el panel izquierdo de la consola

**Navegación paso a paso:**

| Botón | Acción                         |
|-------|--------------------------------|
| ⏮     | Ir al primer paso              |
| ◀ PASO | Retroceder un paso             |
| PASO ▶ | Avanzar un paso               |
| ⏭     | Ir al último paso (resultado)  |

En cada paso se resaltan:
- **Tabla ACTION/GOTO**: celda correspondiente al estado actual + token
- **Panel derecho**: pila visual + input restante + acción tomada

---

### 4. Explorar los estados

Haz clic en el tab **ESTADOS** y luego en cualquier estado `Iₙ`  
para verlo seleccionado. La tabla ACTION/GOTO resalta la fila de ese estado.

---

## API — endpoints disponibles

### `GET /health`
```json
{ "status": "ok", "service": "syntra-api" }
```

### `POST /api/parser/compile`
```json
// Request
{ "content": "%token c d\n%%\nS : C C ;\nC : c C | d ;\n" }

// Response — datos completos del análisis LR(1)
{
  "states":        [ { "id": 0, "items": ["[S' -> • S, $]", ...] } ],
  "action":        { "0": { "c": "s2", "d": "s3" }, "1": { "$": "acc" } },
  "goto":          { "0": { "S": 1, "C": 4 } },
  "terminals":     ["$", "c", "d"],
  "non_terminals": ["S", "C"],
  "first":         { "S": ["c","d"], "C": ["c","d"] },
  "follow":        { "S": ["$"], "C": ["$","c","d"] },
  "prods":         [ { "n": 1, "lhs": "S", "rhs": ["C","C"] } ],
  "problems":      [ { "level": "info", "code": "I100", "msg": "..." } ],
  "start_symbol":  "S"
}
```

### `POST /api/parser/parse`
```json
// Request
{ "content": "...", "tokens": ["c", "c", "d", "c", "d"] }

// Response — traza paso a paso
{
  "trace": [
    {
      "stack":     [0],
      "remaining": ["c","c","d","c","d","$"],
      "action":    "s2",
      "desc":      "Estado 0, símbolo 'c' → Shift a I2"
    }
  ],
  "accepted": true,
  "error":    null
}
```

---

## Formato del archivo .yalp

```
/* comentarios con /* ... */ */

%token TOKEN_A TOKEN_B TOKEN_C

%%

produccion_inicial : TOKEN_A produccion_b TOKEN_C ;

produccion_b : TOKEN_B produccion_b
             | TOKEN_A
             ;
```

- `%token` declara los terminales (tokens del léxico)
- `%%` separa la cabecera de las producciones
- Las producciones usan `:` y terminan con `;`
- Las alternativas se separan con `|`
- Los no-terminales son cualquier identificador NO declarado en `%token`

---

## Cadenas válidas para la gramática de ejemplo

Gramática: `S → C C`, `C → c C | d` (genera `c^n d c^n d`)

| Cadena        | Válida |
|---------------|--------|
| `d d`         | ✓      |
| `c d c d`     | ✓      |
| `c c d c c d` | ✓      |
| `c d`         | ✗ (solo una C) |
| `c d c`       | ✗ (segunda C incompleta) |
| `d c d`       | ✗      |
