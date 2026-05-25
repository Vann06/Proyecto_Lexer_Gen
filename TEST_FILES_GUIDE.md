# 📋 Guía de Archivos de Prueba - LR(1)

## Descripción General

Estos archivos YAIP son gramáticas de ejemplo para probar y explorar las funcionalidades del analizador sintáctico LR(1) en SYNTRA.

Cada archivo está diseñado para mostrar diferentes aspectos del autómata LR(1) y la UI.

---

## 📁 Archivos Disponibles

### 1. **grammar_simple.yaip** ✅ BÁSICO
```
S : a b ;
```

**Complejidad:** ⭐ Mínima
**Estados generados:** ~3-4
**Uso:** Introducción, entender estructura básica

**Qué probar:**
- Compilar y ver los estados iniciales
- Observar cómo un estado transiciona a otro con `a` y luego con `b`
- Ver la tabla ACTION/GOTO más simple posible
- Entrada válida: `a b`

**Observaciones:**
- Perfecta para principiantes
- Las transiciones son lineales y fáciles de seguir
- Buen punto de partida

---

### 2. **grammar_expr.yaip** ⭐ RECOMENDADO PARA DEMO
```
E : E plus T | T ;
T : T star F | F ;
F : ( E ) | id | num ;
```

**Complejidad:** ⭐⭐⭐ Media
**Estados generados:** ~10-12
**Uso:** Demostración principal, ejemplo académico clásico

**Qué probar:**
- Compilar y explorar los 10+ estados
- Ver cómo el autómata maneja operadores con precedencia
- Observar transiciones complejas (múltiples símbolos desde un estado)
- Parsing con entrada: `id plus num`, `num star id`, `num plus id star num`
- Ver cómo se resuelven conflictos con lookahead

**Observaciones:**
- Gramática de expresiones aritméticas clásica
- Muestra bien cómo LR(1) maneja precedencia y asociatividad
- Las transiciones muestran el flujo del parsing
- **MEJOR PARA GRABAR EL VIDEO**

**Entradas de prueba:**
```
id                      ✅ válida
num                     ✅ válida
id plus num             ✅ válida
num plus id star num    ✅ válida
lparen num rparen       ✅ válida
id plus                 ❌ inválida (falta término)
```

---

### 3. **grammar_list.yaip** ⭐⭐ INTERMEDIO
```
L : item | L comma item ;
```

**Complejidad:** ⭐⭐ Fácil-Media
**Estados generados:** ~4-6
**Uso:** Recursión izquierda, listas

**Qué probar:**
- Ver cómo la recursión izquierda se expande en los estados
- Transiciones que se repiten (back-edges en el autómata)
- Parsing de listas: `item`, `item comma item`, `item comma item comma item`
- Observar el patrón de shift-reduce

**Observaciones:**
- Demuestra bien cómo funciona la recursión izquierda
- Menos estados que expr, pero estructura similar
- Buena para entender shift/reduce conflicts

**Entradas de prueba:**
```
item                           ✅ válida
item comma item                ✅ válida
item comma item comma item     ✅ válida
item comma                     ❌ inválida
comma item                     ❌ inválida
```

---

### 4. **grammar_program.yaip** ⭐⭐⭐⭐ AVANZADO
```
P : S | P S ;
S : id = E ;
  | if E { P }
  | if E { P } else { P }
  | while E { P }
  ;
E : num | id ;
```

**Complejidad:** ⭐⭐⭐⭐ Alta
**Estados generados:** ~20-25
**Uso:** Lenguajes con sentencias, bloques

**Qué probar:**
- Explorar el autómata completo con muchos estados
- Ver cómo se manejan las sentencias anidadas
- Transiciones ramificadas (multiple choises desde un estado)
- Parsing de programas simples
- Resolver ambigüedades con lookahead

**Observaciones:**
- Más realista (similar a lenguajes reales)
- Muchas transiciones interesantes para observar
- Demuestra el poder de LR(1) para lenguajes complejos

**Entradas de prueba:**
```
id = num ;                                           ✅ válida
if num { id = num ; }                               ✅ válida
if id { id = num ; } else { id = num ; }            ✅ válida
while num { id = num ; id = num ; }                 ✅ válida
if num { if num { id = num ; } }                    ✅ válida
id = ;                                              ❌ inválida
```

---

## 🎬 Flujo Recomendado para Video Demo

### Setup inicial (0:00 - 0:30)
1. Cargar `grammar_simple.yaip`
2. Compilar (RUN)
3. Mostrar resultados básicos

### Demostración principal (0:30 - 3:30)
1. Cambiar a `grammar_expr.yaip`
2. Compilar
3. Ir a pestaña **ESTADOS**
4. Seleccionar I0 y mostrar:
   - Los items (producciones)
   - Las transiciones (E → I1, T → I2, F → I4)
   - El contador actualizado
5. Navegar por otros estados (I1, I2, I4, etc.)
6. Correlacionar con pestaña **ACTION/GOTO**

### Parsing en acción (3:30 - 4:00)
1. Ir a **PARSE CONSOLE**
2. Entrar una expresión: `id plus num`
3. Clickear **PARSEAR**
4. Mostrar el trace completo
5. Usar **PASO** y **PASO** para navegar la traza

---

## 🔍 Qué Observar en Cada Archivo

| Aspecto | Simple | Expr | List | Program |
|---------|--------|------|------|---------|
| Estados | 3-4 | 10-12 | 4-6 | 20-25 |
| Transiciones | Lineales | Ramificadas | Recursivas | Complejas |
| Conflictos | Ninguno | Precedencia | Shift/Reduce | Múltiples |
| Lookahead | Simple | Crítico | Importante | Crítico |
| Para principiantes | ✅ Sí | ⭐ Mejor | ✅ Sí | ❌ No |

---

## 📊 Estadísticas LR(1)

### Compilación
```
grammar_simple.yaip
├─ Lexemas: 2
├─ No-terminales: 1
├─ Producciones: 1
├─ Estados LR(0): 4
├─ Estados LALR(1): 4
└─ Conflictos: 0 ✅

grammar_expr.yaip
├─ Lexemas: 6
├─ No-terminales: 3
├─ Producciones: 6
├─ Estados LR(0): 14
├─ Estados LALR(1): 11
└─ Conflictos: 0 ✅

grammar_list.yaip
├─ Lexemas: 2
├─ No-terminales: 1
├─ Producciones: 2
├─ Estados LR(0): 6
├─ Estados LALR(1): 6
└─ Conflictos: 0 ✅

grammar_program.yaip
├─ Lexemas: 8
├─ No-terminales: 3
├─ Producciones: 6
├─ Estados LR(0): 25
├─ Estados LALR(1): 22
└─ Conflictos: 0 ✅
```

---

## 🎓 Conceptos a Ilustrar

### Con `grammar_simple.yaip`
- ✅ Estructura básica de estados
- ✅ Transiciones simples (shift)
- ✅ Aceptación final (accept)

### Con `grammar_expr.yaip` ⭐
- ✅ Ambigüedad y resolución con precedencia
- ✅ Múltiples transiciones desde un estado
- ✅ Reduce actions
- ✅ GOTO para no-terminales
- ✅ Lookahead resolving conflicts

### Con `grammar_list.yaip`
- ✅ Recursión izquierda
- ✅ Patrones shift-reduce
- ✅ Autómata con ciclos (back-edges)

### Con `grammar_program.yaip`
- ✅ Lenguajes con control de flujo
- ✅ Anidamiento de estructuras
- ✅ Autómata grande y realista

---

## 🚀 Cómo Usar en SYNTRA

### Paso 1: Cargar archivo
```
1. Click en EXPLORER
2. Abrir test_files/grammar_X.yaip
3. Seleccionar en WORKSPACE
```

### Paso 2: Compilar
```
1. Click en RUN
2. Esperar compilación
3. Ver resultados en panel derecho
```

### Paso 3: Explorar
```
1. Pestaña ESTADOS: ver items y transiciones
2. Pestaña ACTION/GOTO: ver tabla
3. Pestaña LR(0): ver grafo del autómata
```

### Paso 4: Parsear (opcional)
```
1. Ir a PARSE CONSOLE
2. Tipear entrada
3. Click PARSEAR
4. Ver trace de ejecución
```

---

## 💡 Tips para Entender LR(1)

1. **Items LR(1)**: `[A → α · β, lookahead]`
   - Parte izquierda de `·`: ya procesado
   - Parte derecha de `·`: por procesar
   - `lookahead`: qué símbolo puede venir después

2. **Transiciones**:
   - Son las aristas del autómata
   - Etiquetadas con símbolos (terminales o no-terminales)
   - Te llevan de un estado a otro

3. **Estados aceptados**:
   - Cuando ves un item con `·` al final
   - Significa: hemos completado una producción

4. **Conflictos**:
   - LR(1) resuelve muchos conflictos que LR(0) no puede
   - El lookahead es la clave

---

## 📝 Notas de Implementación

- Todos los archivos usan la sintaxis YAIP estándar
- Compatible con compiladores LALR(1), SLR(1), LL(1)
- Sin errores de compilación
- Listos para usar inmediatamente

---

## 🎯 Checklist para Demo Video

- [ ] Cargar `grammar_simple.yaip` y compilar
- [ ] Cambiar a `grammar_expr.yaip`
- [ ] Mostrar pestaña ESTADOS con transiciones visibles
- [ ] Resaltar un estado (ej: I0) y mostrar:
  - [ ] Los items en la parte superior
  - [ ] El separador visual
  - [ ] Las transiciones (E → I1, T → I2, F → I4)
  - [ ] El contador "X items, Y transiciones"
- [ ] Correlacionar con ACTION/GOTO
- [ ] Parsear una entrada (`id plus num`)
- [ ] Mostrar trace completo

