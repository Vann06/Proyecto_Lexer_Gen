# ⚡ Quick Demo Guide - 4 Minutos

## 📌 Checklist Pre-Demo

- [ ] SYNTRA abierto en http://localhost:8000
- [ ] Test files descargados en `test_files/`
- [ ] Script (`SCRIPT_DEMO_LR1.md`) a mano
- [ ] Entrada lista: `id plus num`

---

## 🎬 Timeline Paso a Paso

### [0:00] INTRO (15 seg)

```
Narración: "Hola, te presento SYNTRA - un analizador sintáctico 
LR(1) con IDE visual. Vamos a explorar cómo funciona el autómata 
LR(1) y cómo la UI te muestra el flujo de estados."

Acción: Mostrar pantalla principal
```

**Clicks:**
1. Mostrar pantalla completa del IDE
2. Resaltar secciones: EXPLORER (izq), EDITOR (centro), RESULTS (derecha)

---

### [0:15] LR(1) BÁSICO (90 seg)

#### [0:15-0:45] ¿Qué es LR(1)?

```
Narración: "LR(1) es un analizador descendente que construye 
un autómata de estados. Cada estado contiene items LR(1) - 
producciones parcialmente procesadas con lookahead."

Acción: Cargar grammar_expr.yaip y compilar
```

**Clicks:**
1. EXPLORER → test_files/ → grammar_expr.yaip
2. Se carga en WORKSPACE
3. Botón **RUN** → Esperar compilación
4. Ver "✓ COLECCIÓN CANÓNICA · LR(1) — 11 estados"

#### [0:45-1:30] Estados e Transiciones

```
Narración: "Cada estado es un nodo del autómata. Las transiciones 
indican qué símbolo nos lleva a qué estado siguiente.

Por ejemplo: si estamos en I0 y vemos 'E', saltamos a I1.

Ahora SYNTRA muestra las transiciones disponibles en cada estado - 
qué símbolo entra, qué estado sale. Esto es exactamente lo que ves 
en la pestaña ESTADOS."

Acción: Ir a pestaña ESTADOS y mostrar transiciones
```

**Clicks:**
1. Click en pestaña **ESTADOS**
2. Click en estado **I0** (el primero de la lista)
3. **Zoom/pausa en I0** para ver claramente:
   ```
   ■ I0                    3 items, 3 transiciones
   
   E → · E plus T, {$}
   E → · T, {$}
   T → · T star F, {$}
   ─────────────────────
   E        → I1
   T        → I2
   F        → I4
   ```
4. Señalar:
   - Items arriba
   - Separador visual
   - Transiciones abajo (E→I1, T→I2, F→I4)

#### [1:30-1:45] Variantes LR

```
Narración: "SYNTRA soporta LALR(1), SLR(1) y LL(1). Todos 
comparten la misma estructura básica de estados y transiciones."

Acción: Mostrar los botones de variantes
```

**Clicks:**
1. Mostrar botones **LALR(1)**, **SLR(1)**, **LL(1)** en la parte superior
2. Clickear en SLR(1) y mostrar que se recompila
3. Volver a LALR(1)

---

### [1:45] UI - TRANSICIONES (75 seg)

#### [1:45-2:20] Items + Transiciones Visualizadas

```
Narración: "En la pestaña ESTADOS, cada estado ahora muestra 
tres cosas: 
1. Sus items - producciones con el punto
2. Un separador visual
3. Las transiciones salientes

Esto te ayuda a entender el flujo sin saltar entre pestañas."

Acción: Mostrar varios estados con zoom
```

**Clicks:**
1. En pestaña ESTADOS, seleccionar **I0** (ya mostrado)
2. Scroll down, seleccionar **I1** y mostrar:
   ```
   ■ I1                    1 items, 0 transiciones
   
   E → E · plus T, {$}
   ```
3. Seleccionar **I2** y mostrar:
   ```
   ■ I2                    1 items, 0 transiciones
   
   E → T ·, {$}
   ```
4. Seleccionar **I4** y mostrar más transiciones:
   ```
   ■ I4                    3 items, 2 transiciones
   
   F → · ( E )
   F → · id
   F → · num
   ─────────────────────
   (        → I8
   id       → I5
   num      → I6
   ```

#### [2:20-2:50] Correlación con ACTION/GOTO

```
Narración: "Las transiciones que ves aquí se reflejan en la 
tabla ACTION/GOTO. Esta tabla es lo que usa el parser para 
tomar decisiones mientras lee la entrada."

Acción: Mostrar correlación entre ESTADOS y ACTION/GOTO
```

**Clicks:**
1. Pestaña **ESTADOS** → Mostrar I0
2. Cambiar a pestaña **ACTION/GOTO**
3. Mostrar fila para estado 0 en la tabla
4. Señalar cómo los números coinciden:
   - I0 en ESTADOS → fila 0 en ACTION/GOTO
   - I1 en ESTADOS → columna "E" en fila 0 = "1"
   - I2 en ESTADOS → columna "T" en fila 0 = "2"

#### [2:50-3:00] Navegación Visual

```
Narración: "La UI está diseñada para aprendizaje. Puedes ver 
items, entender transiciones, y simular parsing en tiempo real."

Acción: Mostrar LR(0) para vista completa del autómata
```

**Clicks:**
1. Pestaña **LR(0)**
2. Ver grafo completo con todos los estados y transiciones
3. Mostrar cómo corresponde con lo visto en ESTADOS

---

### [3:00] DEMO PRÁCTICO (60 seg)

#### [3:00-3:30] Compilación y Setup

```
Narración: "Ahora vamos a simular el parsing de una expresión. 
Vemos la compilación exitosa con 11 estados."

Acción: Mostrar estado compilado
```

**Clicks:**
1. Ya está compilado de antes
2. Confirmar en panel derecho: "✓ COMPILACIÓN EXITOSA"
3. Mostrar: "11 estados"

#### [3:30-3:50] Parsing en Acción

```
Narración: "Vamos a parsear la entrada: 'id plus num'

Esto es una expresión simple: identificador PLUS número.
El parser va a:
1. Leer 'id'
2. Leer 'plus'
3. Leer 'num'
4. Reducir usando las reglas
5. Aceptar"

Acción: Ejecutar parsing y mostrar trace
```

**Clicks:**
1. Ir a **PARSE CONSOLE** (abajo de la pantalla)
2. Limpiar entrada anterior (si existe)
3. Tipear entrada: `id plus num`
4. Clickear botón **▶ PARSEAR**
5. Esperar a que se complete
6. Mostrar **ESTADO ACTUAL** final: "✅ ACCEPT"

**Trace esperado:**
```
Step 1: [0] id → shift → [0,5]
Step 2: [0,5] plus → reduce F → [0,4]
Step 3: [0,4] plus → reduce T → [0,2]
Step 4: [0,2] plus → shift → [0,2,7]
Step 5: [0,2,7] num → shift → [0,2,7,5]
Step 6: [0,2,7,5] $ → reduce F → [0,2,7,4]
Step 7: [0,2,7,4] $ → reduce T → [0,2,7,8]
Step 8: [0,2,7,8] $ → reduce E → [0,1]
Step 9: [0,1] $ → accept
```

#### [3:50-4:00] Cierre

```
Narración: "Con SYNTRA, los autómatas LR(1) pasan de ser algo 
abstracto a visual e interactivo. 

Ves exactamente dónde estás en el autómata, qué transiciones 
disponibles tienes, y cómo el parser toma decisiones paso a paso.

Perfecto para aprender parsing formal. Prueba con tus propias 
gramáticas usando archivos .yaip"

Acción: Mostrar pantalla completa una última vez
```

**Visuals:**
1. Mostrar todo el IDE
2. Resaltar las tres pestañas principales: ESTADOS, ACTION/GOTO, LR(0)
3. Fade out

---

## 🎥 Versión Corta (2 minutos)

Si necesitas una versión más rápida:

### [0:00-0:30] Intro + Compilación
- Mostrar IDE
- Cargar grammar_expr.yaip
- Compilar (RUN)

### [0:30-1:15] Items + Transiciones
- Pestaña ESTADOS
- Mostrar I0 con items y transiciones
- Explicar qué significa cada parte

### [1:15-1:45] ACTION/GOTO
- Cambiar a ACTION/GOTO
- Mostrar correlación con ESTADOS

### [1:45-2:00] Parsing
- Parsear `id plus num`
- Mostrar resultado ACCEPT

---

## 🔑 Puntos Clave para Enfatizar

1. **Items LR(1)**: Muestran progreso de parsing
2. **Transiciones**: Definen el flujo del autómata
3. **Lookahead**: Resuelve ambigüedades
4. **UI Visual**: Integra todo en un lugar

---

## 📸 Screenshots Importantes

| Momento | Pantalla | Duración |
|---------|----------|----------|
| 0:15 | IDE completo | 10 seg |
| 0:45 | Estado I0 con transiciones | 15 seg |
| 1:45 | Varios estados (I0, I2, I4) | 20 seg |
| 2:20 | ESTADOS + ACTION/GOTO lado a lado | 15 seg |
| 2:50 | Grafo LR(0) completo | 10 seg |
| 3:00 | Panel de compilación | 5 seg |
| 3:30 | PARSE CONSOLE con entrada | 20 seg |
| 3:50 | Trace completo | 10 seg |

---

## ⏰ Timing Exacto (sin pausas)

```
[0:00] INTRO
└─ [0:15] Mostrar IDE

[0:15] LR(1) BÁSICO
├─ [0:15] Cargar grammar_expr.yaip
├─ [0:30] Compilar
├─ [0:45] Explicar transiciones
└─ [1:30] Variantes LR

[1:45] UI - TRANSICIONES
├─ [1:45] Items + Transiciones en I0
├─ [2:20] Varios estados (I1, I2, I4)
├─ [2:30] ACTION/GOTO correlación
└─ [2:50] LR(0) grafo

[3:00] PARSING
├─ [3:00] Setup
├─ [3:30] Ejecutar `id plus num`
├─ [3:50] Mostrar trace
└─ [4:00] Cierre
```

---

## 💾 Archivos que Necesitas

```
✅ SCRIPT_DEMO_LR1.md      (guión completo con tiempos)
✅ TEST_FILES_GUIDE.md      (detalles de archivos de prueba)
✅ QUICK_DEMO_GUIDE.md      (este archivo)

✅ test_files/grammar_expr.yaip    (gramática para demo)
✅ test_files/input_expr.txt       (entradas de ejemplo)
```

---

## 🚀 Pro Tips

1. **Zoom**: Usa zoom en elementos pequeños para que se vea mejor
2. **Pausas**: Deja 2-3 segundos al cambiar de pantalla
3. **Narración**: Habla claro, 140-160 palabras/minuto
4. **Velocidad**: Si no terminas en 4 min, cortaparte el LR(0) grafo
5. **Backup**: Graba una entrada extra de `num star id` por si acaso

---

## ✅ Checklist Final

- [ ] Todos los archivos en su lugar
- [ ] Script impreso o en pantalla auxiliar
- [ ] Entrada `id plus num` copiada y lista
- [ ] Botones RUN, PARSEAR probados
- [ ] Micrófono funciona
- [ ] Resolución de pantalla es 1280x800 o superior
- [ ] Sin distracciones en background
- [ ] Recording software configurado

**¡Listo para grabar!**

