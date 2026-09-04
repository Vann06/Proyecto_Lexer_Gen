# Batería de colecciones

`colecciones_casos.txt` cubre las **cuatro colecciones** del analizador —
arreglo, conjunto, mapa y tupla — con un caso exitoso y uno fallido por regla.

Va con su propia gramática, `colecciones.yal` + `colecciones.yalp`, porque
**Compiscript no tiene sintaxis de mapa, conjunto ni tupla**. Ese es justamente
el punto: demuestra que el soporte de colecciones del analizador es
*configuración del `.yalp`*, no código atado a un lenguaje. Las mismas reglas de
`src/semantico/collections` validan esta gramática sin una sola línea de Rust
específica de ella.

## Cómo correrla desde el IDE

1. Levanta el backend: desde la raíz, `docker compose up --build`.
2. Abre `http://localhost:4000` y carga `workspace/colecciones.yal`,
   `workspace/colecciones.yalp` y `workspace/colecciones_casos.txt`.
3. Selecciona LALR(1) o SLR(1) y pulsa **RUN**.
4. Pulsa **PARSEAR** y ve seleccionando cada caso en el panel de la izquierda.
5. En **PROBLEMAS** salen los códigos; el gutter marca la línea.

## La sintaxis de esta gramática

| Colección | Tipo | Literal | Indexado |
|---|---|---|---|
| Arreglo | `entero[]` | `[1, 2, 3]` | `a[0]` con entero |
| Conjunto | `conj<entero>` | `conj{ 1, 2 }` | **no es indexable** |
| Mapa | `mapa<texto, entero>` | `mapa{ "a": 1 }` | `d["a"]` con la clave declarada |
| Tupla | `tupla<texto, entero>` | `tupla( "x", 1 )` | `t[0]` con literal constante |

Cada literal lleva un token marcador al frente (`conj`, `mapa`, `tupla`). No es
decorativo: el analizador reconoce los literales **por forma** —"¿tengo este
token entre mis hijos?"— así que dos literales que compartieran delimitador
serían indistinguibles. De paso evita la ambigüedad LALR de una tupla `( ... )`
contra la expresión entre paréntesis.

## Qué comprueba cada caso

| # | Regla | Esperado |
|---|---|---|
| 1 | Arreglo homogéneo e indexado | ✅ |
| 2 | Arreglo con elementos heterogéneos | `S032` |
| 3 | Arreglo indexado con algo que no es entero | `S033` |
| 4 | Arreglo bidimensional, indexado dos veces | ✅ |
| 5 | Conjunto homogéneo | ✅ |
| 6 | Conjunto con elementos heterogéneos | `S032` |
| 7 | Un conjunto **no** es indexable | `S034` |
| 8 | `conj<entero>` asignado a `conj<texto>` | `S006` |
| 9 | Mapa con acceso por clave correcta | ✅ |
| 10 | Mapa indexado con una clave del tipo equivocado | `S037` |
| 11 | Mapa con claves heterogéneas | `S032` |
| 12 | Mapa con valores heterogéneos | `S032` |
| 13 | El acceso devuelve el tipo del **valor**, no el de la clave | `S006` |
| 14 | Tupla con tipos mezclados — lo normal en una tupla | ✅ |
| 15 | `t[1]` es `entero` | ✅ |
| 16 | La misma tupla en otra posición es otro tipo: `t[0]` es `texto` | `S006` |
| 17 | Índice literal fuera del rango de la tupla | `S038` |
| 18 | Iterar un conjunto recorre sus elementos | ✅ |
| 19 | Iterar un mapa recorre sus **claves** | ✅ |
| 20 | Una tupla no es iterable: es heterogénea | `S036` |
| 21 | Mapa de arreglos: colecciones anidadas | ✅ |
| 22 | Dos conjuntos de tipos distintos no son compatibles | `S006` |

## Decisiones de diseño que conviene conocer

- **Un conjunto no se indexa.** Es la diferencia observable entre `Set(T)` y
  `Array(T)`: mismos elementos, pero sin orden ni claves.
- **Iterar un mapa da sus claves**, como en Python o JavaScript. Iterar los
  valores sería otra operación.
- **Una tupla no es iterable**: al ser heterogénea no existe "el" tipo de sus
  elementos que un `foreach` pudiera ofrecer.
- **Indexar una tupla exige un literal constante.** `t[0]` y `t[1]` devuelven
  tipos distintos, así que sin saber el valor no hay tipo que dar. Con un índice
  variable (`t[i]`) el tipo queda desconocido **en silencio**, sin error: es la
  misma política de "no sabemos, no inventamos" del resto del analizador.
- **Los compuestos son compatibles solo consigo mismos**, sin varianza —
  `conj<entero>` no acepta un `conj<texto>` ni un `entero[]`.

## Baterías relacionadas

- `casos_semanticos.txt` — 45 casos de todas las reglas sobre Compiscript.
- `duplicates_casos.txt` — 15 casos de duplicados y símbolos sin usar.
