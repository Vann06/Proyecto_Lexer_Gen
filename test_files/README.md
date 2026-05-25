# 🧪 Test Files para SYNTRA - Analizador LR(1)

## 📂 Estructura

```
test_files/
├── grammar_simple.yaip       # Gramática básica (recomendado para comenzar)
├── grammar_expr.yaip         # Expresiones aritméticas (para demo principal)
├── grammar_list.yaip         # Listas recursivas
├── grammar_program.yaip      # Programa simple con control de flujo
├── input_simple.txt          # Entradas para grammar_simple
├── input_expr.txt            # Entradas para grammar_expr
├── input_list.txt            # Entradas para grammar_list
├── input_program.txt         # Entradas para grammar_program
└── README.md                 # Este archivo
```

---

## 🚀 Cómo Usar

### 1. Cargar una Gramática en SYNTRA

```bash
# Opción A: Desde el explorador en la UI
1. Click en "EXPLORER"
2. Navega a test_files/
3. Abre grammar_expr.yaip
4. Aparecerá en WORKSPACE

# Opción B: Copiar/pegar manualmente
1. Abre el archivo .yaip en un editor
2. Copia el contenido
3. Pega en el editor de SYNTRA
```

### 2. Compilar la Gramática

```
1. Click en el botón "RUN" (arriba a la derecha)
2. Espera a que se compile
3. Deberías ver "✓ COMPILACIÓN EXITOSA" si no hay errores
```

### 3. Explorar los Resultados

**Panel de RESULTADOS (derecha):**
- **ESTADOS**: Ver los estados del autómata LR(1) con sus transiciones
- **ACTION/GOTO**: Tabla de acciones y saltos
- **LR(0)**: Grafo visual del autómata

### 4. Parsear una Entrada

```
1. Copia una línea de input_*.txt
2. Pégala en "PARSE CONSOLE" (abajo)
3. Click en "▶ PARSEAR"
4. Observa el TRACE DE EJECUCIÓN:
   - STACK: pila del parser
   - INPUT: tokens restantes
   - ACTION: acción que se ejecutó
```

---

## 📊 Archivos por Complejidad

| Archivo | Complejidad | Uso | Estados | Conflictos |
|---------|-------------|-----|---------|-----------|
| `grammar_simple.yaip` | ⭐ Mínima | Aprender | 4 | 0 ✅ |
| `grammar_expr.yaip` | ⭐⭐⭐ Media | **Demo** | 11 | 0 ✅ |
| `grammar_list.yaip` | ⭐⭐ Fácil | Recursión | 6 | 0 ✅ |
| `grammar_program.yaip` | ⭐⭐⭐⭐ Alta | Avanzado | 22 | 0 ✅ |

---

## 🎯 Recomendado para Video Demo

### Archivo: `grammar_expr.yaip`

**Por qué:**
- Complejidad media (no muy simple, no muy complicado)
- Muestra bien el autómata LR(1)
- Ejemplo académico clásico
- Visualmente interesante

**Entradas recomendadas:**
```
id plus num           # expresión simple
num star id           # otro símbolo
id plus id star num   # precedencia en acción
lparen num rparen     # paréntesis
```

---

## 🔍 Qué Observar en Cada Pestaña

### Pestaña: ESTADOS ⭐ NUEVA

**Muestra:**
- Items LR(1) de cada estado
- Transiciones disponibles (NEW!)
- Contador de items y transiciones

**Ejemplo para Estado I0:**
```
■ I0                    3 items, 3 transiciones

E → · E plus T, {$}
E → · T, {$}
T → · T star F, {$}
—————————————————
E        → I1
T        → I2
F        → I4
```

**Qué significa:**
- Los items muestran dónde está el punto (•) 
- Las transiciones muestran: si ves este símbolo → vas a este estado
- Ejemplo: Si ves token `E` → salta a estado `I1`

### Pestaña: ACTION/GOTO

**Muestra:**
- Tabla de decisiones del parser
- Para cada estado e input: qué hacer (SHIFT, REDUCE, ACCEPT)
- Transiciones GOTO para no-terminales

**Cómo leer:**
```
Fila: Estado actual
Columna: Token/símbolo actual
Celda: Acción (s3 = shift a 3, r1 = reduce 1, accept)
```

### Pestaña: LR(0)

**Muestra:**
- Grafo visual del autómata
- Nodos = estados
- Flechas = transiciones etiquetadas
- Permite ver todo el flujo de golpe

---

## 💡 Conceptos Clave para Entender

### Items LR(1)
```
A → α · β , lookahead

Significado:
- A → α · β : producción con punto de parsing
  - α : ya procesado
  - β : por procesar
- lookahead : qué token puede venir después
```

### Transiciones
```
Arista del autómata de un estado a otro.
Etiquetada con un símbolo (terminal o no-terminal).

Ejemplo: E → I1
- Si ves token 'E' → ir al estado I1
```

### Parseo
```
Proceso de lectura de tokens y construcción del árbol.
El parser sigue las transiciones del autómata según los tokens.
```

---

## ✅ Ejemplos de Salida Esperada

### grammar_simple.yaip

**Compilación:**
```
COLECCIÓN CANÓNICA · LR(1) — 4 estados
Léxemas: 2 (a, b)
No-terminales: 1 (S)
Conflictos: 0 ✅
```

**Parsing `a b`:**
```
Entrada válida ✅
Trace:
[0] a → shift → [0,1]
[0,1] b → shift → [0,1,3]
[0,1,3] $ → reduce S → [0,2]
[0,2] $ → accept
```

### grammar_expr.yaip

**Compilación:**
```
COLECCIÓN CANÓNICA · LR(1) — 11 estados
Léxemas: 6
No-terminales: 3 (E, T, F)
Conflictos: 0 ✅
```

**Parsing `id plus num`:**
```
Entrada válida ✅
Trace:
[0] id → shift → [0,5]
[0,5] plus → reduce F → [0,4]
[0,4] plus → reduce T → [0,2]
[0,2] plus → shift → [0,2,7]
[0,2,7] num → shift → [0,2,7,5]
[0,2,7,5] $ → reduce F → [0,2,7,4]
[0,2,7,4] $ → reduce T → [0,2,7,8]
[0,2,7,8] $ → reduce E → [0,1]
[0,1] $ → accept
```

---

## 🎓 Flujo de Aprendizaje Recomendado

1. **Principiante**: Comienza con `grammar_simple.yaip`
   - Entiende la estructura básica
   - Ve cómo funcionan los estados
   - Sigue las transiciones paso a paso

2. **Intermedio**: Pasa a `grammar_list.yaip` o `grammar_expr.yaip`
   - Observa más estados
   - Entiende la recursión
   - Ve cómo se resuelven decisiones con lookahead

3. **Avanzado**: Explora `grammar_program.yaip`
   - Autómata grande y realista
   - Múltiples transiciones
   - Anidamiento de estructuras

---

## 🐛 Troubleshooting

### "Error: No se puede compilar la gramática"
- Verifica que el archivo tenga sintaxis YAIP correcta
- Revisa que los tokens estén declarados con `%token`
- Asegúrate de que `%%` separe tokens de reglas

### "Estado indefinido en la tabla"
- Esto significa que el parser llegó a un estado inesperado
- Verifica tu entrada contra la gramática
- Mira el TRACE para ver dónde falló

### "Reduce/Reduce conflict"
- Indica ambigüedad en la gramática
- Algunas gramáticas tienen conflictos LR(1) inherentes
- SLR(1) o LL(1) podrían fallar donde LALR(1) tiene éxito

---

## 📚 Recursos Adicionales

Para entender mejor LR(1):
- Ver `SCRIPT_DEMO_LR1.md` para guión de video
- Leer `TEST_FILES_GUIDE.md` para análisis detallado
- Consultar paneles de SYNTRA:
  - Pestaña "ACTION/GOTO": decisiones del parser
  - Pestaña "LR(0)": visualización del autómata

---

## 🎬 Para Grabar Video Demo

1. Cargar `grammar_expr.yaip`
2. Compilar (RUN)
3. Ir a pestaña ESTADOS
4. Mostrar Estado I0 con sus:
   - Items (producciones)
   - Separador visual
   - Transiciones (E→I1, T→I2, F→I4)
5. Navegar a otros estados para mostrar el flujo
6. Correlacionar con ACTION/GOTO
7. Parsear entrada: `id plus num`
8. Mostrar el trace completo

---

## 📝 Notas

- Todos los archivos .yaip son válidos y sin errores
- Los archivos .txt contienen entradas de ejemplo (válidas e inválidas)
- Compatible con LALR(1), SLR(1), LL(1)
- Listos para uso inmediato

