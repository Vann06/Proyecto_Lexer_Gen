# 📹 Script Demo LR(1) + UI - 4 minutos

## Estructura del Video
- **Intro**: 15 seg
- **LR(1) Explicación**: 90 seg
- **UI y Transiciones**: 75 seg
- **Demo Práctico**: 60 seg

---

## [0:00 - 0:15] INTRO

**Narración:**
"Hola, te presento SYNTRA - un analizador sintáctico LR(1) con IDE visual. Vamos a explorar cómo funciona el autómata LR(1) y cómo la UI te muestra el flujo de estados en tiempo real."

**Visuals:**
- Mostrar pantalla principal del IDE
- Resaltar las tres secciones principales

---

## [0:15 - 1:45] LR(1) - EXPLICACIÓN TÉCNICA

### [0:15 - 0:45] ¿Qué es LR(1)?

**Narración:**
"LR(1) es un analizador sintáctico descendente que construye un autómata de estados. Cada estado representa un conjunto de items LR(1) - producciones parcialmente procesadas con un símbolo de lookahead.

Cuando compilas una gramática, SYNTRA genera:
- Una colección canónica de estados
- Transiciones entre estados basadas en símbolos terminales y no-terminales
- Una tabla ACTION/GOTO que guía el parsing"

**Visuals:**
- Mostrar seleccionar archivo `grammar_expr.yaip`
- Clickear en RUN para compilar
- Mostrar la tabla ACTION/GOTO generada

### [0:45 - 1:30] Estados e Transiciones

**Narración:**
"Cada estado es un nodo del autómata. Las transiciones indican qué símbolo nos lleva a qué estado siguiente.

Por ejemplo: si estamos en I0 y vemos una 'E', saltamos a I1. Si vemos una 'T', saltamos a I2.

Esto es exactamente lo que ves en la pestaña ESTADOS: cada estado muestra sus items (las producciones) y AHORA también muestra las transiciones disponibles - qué símbolo de entrada te lleva a qué estado."

**Visuals:**
- Ir a pestaña ESTADOS
- Resaltar el estado I0
- Mostrar los items y las transiciones (E → I1, T → I2, etc.)
- Scroll por varios estados

### [1:30 - 1:45] Diferencia LR(1) vs LALR

**Narración:**
"SYNTRA soporta tres variantes:
- LALR(1): la más compacta, el estándar de la industria
- SLR(1): simple pero con menos poder
- LL(1): top-down alternativo

Todos comparten la misma estructura de estados y transiciones."

**Visuals:**
- Mostrar los botones LALR(1), SLR(1), LL(1)
- Cambiar entre ellos para mostrar que la estructura es similar

---

## [1:45 - 3:00] UI - TRANSICIONES EN ACCIÓN

### [1:45 - 2:20] Nueva Visualización: Items + Transiciones

**Narración:**
"Aquí está la mejora: en la pestaña ESTADOS, cada estado ahora muestra:

1. Sus items - las producciones con el punto (•) indicando dónde estamos
2. Un separador visual
3. Las transiciones salientes - qué símbolo nos lleva a qué estado

Esto te ayuda a entender el flujo sin saltar entre pestañas."

**Visuals:**
- Hacer zoom en un estado, ej. I0
- Mostrar items en la parte superior
- Mostrar línea separadora
- Mostrar transiciones (E → I1, T → I2, F → I4)
- Mostrar el contador actualizado: "3 items, 3 transiciones"

### [2:20 - 2:50] Correlación con ACTION/GOTO

**Narración:**
"Las transiciones que ves aquí se reflejan en la tabla ACTION/GOTO. 

Cuando parseas, si estás en estado 0 y ves un terminal como 'd', haces SHIFT a estado 4 (el s4 que ves en la tabla).

Si ves un no-terminal como 'C', usas GOTO para ir al estado correspondiente."

**Visuals:**
- Mostrar pestaña ESTADOS con transiciones visibles
- Cambiar a pestaña ACTION/GOTO
- Mostrar cómo los números coinciden (I0 con s4 en la tabla, etc.)
- Usar colores o resalte para mostrar la correlación

### [2:50 - 3:00] Navegación Intuitiva

**Narración:**
"La UI está diseñada para aprendizaje. Puedes:
- Ver los items en cada estado
- Entender las transiciones disponibles
- Simular el parsing paso a paso con el botón PARSEAR"

**Visuals:**
- Mostrar botones PASO y PASO (forward/backward)
- Mostrar PARSE CONSOLE donde se ve el trace

---

## [3:00 - 4:00] DEMO PRÁCTICO

### Setup

**Narración:**
"Vamos a ver un ejemplo práctico. Primero compilamos la gramática."

**Visuals:**
1. Mostrar archivo `grammar_expr.yaip` cargado
2. Clickear RUN
3. Esperar compilación exitosa

### Recorrido por Estados

**Narración:**
"Aquí vemos la colección canónica - 10 estados. Seleccionemos el estado I0, el estado inicial.

Ves:
- Tres items LR(1) que caracterizan este estado
- Tres transiciones: E va a I1, T va a I2, F va a I4

Esto es el GOTO del estado inicial. Dependiendo del símbolo que veamos, entramos a uno de estos tres estados."

**Visuals:**
- Click en I0 en la pestaña ESTADOS
- Zoom/highlight en los items
- Zoom/highlight en las transiciones
- Mostrar I → I1, I → I2, I → I4

### Parsing en Acción

**Narración:**
"Ahora simula parsing. Ponemos la entrada 'd' en la consola y clickeamos PARSEAR."

**Visuals:**
1. Ir a PARSE CONSOLE
2. Tipear entrada (ej: "c d" o "d")
3. Clickear PARSEAR o PASO
4. Ver el STACK cambiar
5. Ver en ESTADOS cómo el estado actual se destaca
6. Mostrar el ESTADO ACTUAL en el panel inferior

### Validación

**Narración:**
"SYNTRA valida la entrada contra la gramática:
- Si es válida, ves la traza completa de parsing
- Los pasos te muestran exactamente qué transiciones se tomaron
- Puedes navegar hacia adelante y atrás para entender cada decisión"

**Visuals:**
- Completar el parsing
- Mostrar ESTADO FINAL con accept o error
- Clickear botones ◀ PASO / PASO ▶ para navegar la traza

---

## [3:50 - 4:00] CIERRE

**Narración:**
"SYNTRA transforma los autómatas LR(1) de algo abstracto a algo visual e interactivo. 

Con la nueva visualización de transiciones, entiendes no solo dónde estás, sino exactamente hacia dónde puedes ir. Perfecto para aprender parsing.

Prueba con tus propias gramáticas usando archivos .yaip."

**Visuals:**
- Mostrar la UI completa una última vez
- Resaltar las tres pestañas: ESTADOS, ACTION/GOTO, LR(0)
- Fade out

---

## ⏱️ Timeline Resumido

| Tiempo | Sección | Duración |
|--------|---------|----------|
| 0:00 | Intro | 15 seg |
| 0:15 | ¿Qué es LR(1)? | 30 seg |
| 0:45 | Estados e Transiciones | 45 seg |
| 1:30 | Variantes LR | 15 seg |
| 1:45 | Items + Transiciones UI | 35 seg |
| 2:20 | ACTION/GOTO Correlación | 30 seg |
| 2:50 | Navegación UI | 10 seg |
| 3:00 | Demo Setup | 20 seg |
| 3:20 | Recorrido Estados | 30 seg |
| 3:50 | Parsing en Acción | 10 seg |
| 3:50 | Cierre | 10 seg |
| **Total** | | **4:00** |

---

## 🎬 Tips de Grabación

1. **Herramientas recomendadas**: OBS Studio, ScreenFlow, QuickTime (macOS)
2. **Resolución**: 1280x800 o superior
3. **Velocidad de voz**: 140-160 palabras por minuto para técnico
4. **Música de fondo** (opcional): sonido ambiental bajo, sin distracciones
5. **Cursor**: Usa zoom en elementos pequeños, mueve lentamente entre secciones
6. **Pauses**: 2-3 segundos entre cambios de escena para que el viewer siga

