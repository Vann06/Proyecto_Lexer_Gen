# Plan UI — IDE Analizador Sintáctico (Tauri + React)

## Stack

| Necesidad | Librería |
|---|---|
| Desktop app | Tauri |
| UI framework | React |
| Editor de código | Monaco Editor |
| Grafos NFA/DFA/Estados | viz.js (recibe `.dot` directo) |
| Tablas ACTION/GOTO | HTML + CSS grid |
| Estado global | Zustand |

---

## Estructura del proyecto

```
src/
├── App.jsx
├── components/
│   ├── Editor/
│   │   ├── CodeEditor.jsx        ← Monaco Editor
│   │   └── FileTabs.jsx          ← tabs: Léxico / Gramática / Texto prueba
│   │
│   ├── Lexer/
│   │   ├── LexerPanel.jsx
│   │   ├── RegexTree.jsx         ← árbol AST regex (viz.js)
│   │   ├── AutomataGraph.jsx     ← NFA / DFA como grafo (viz.js, recibe .dot)
│   │   ├── TransitionTable.jsx   ← matriz [estado][char] → estado
│   │   └── TokenStream.jsx       ← lista tokens: lexema, línea, col
│   │
│   ├── Parser/
│   │   ├── ParserPanel.jsx
│   │   ├── GrammarView.jsx       ← producciones parseadas
│   │   ├── FirstFollowTable.jsx
│   │   ├── AutomataStates.jsx    ← estados LR con sus ítems
│   │   ├── ActionGotoTable.jsx   ← tabla ACTION/GOTO con celda activa
│   │   └── ConflictBanner.jsx    ← alerta S/R o R/R
│   │
│   └── Console/
│       ├── ParseConsole.jsx      ← input de tokens + botón parsear
│       └── StepTrace.jsx         ← pasos: estado + token + acción
│
├── hooks/
│   ├── useLexer.js               ← invoke("compilar_yal")
│   └── useParser.js              ← invoke("compilar_yalp")
│
└── store/
    └── appStore.js               ← Zustand
```

---

## Layout visual

```
┌─────────────────────────────────────────────────────────────────────┐
│  [Léxico]  [Gramática]  [Texto prueba]          [▶ Run]  [▶ Step]  │
├──────────────────────┬──────────────────────────────────────────────┤
│                      │ [Gramática][FIRST][FOLLOW][Estados][ACTION]  │
│   CodeEditor.jsx     │ [GOTO][Tokens][DFA][Código gen.]             │
│                      ├──────────────────────────────────────────────┤
│   Monaco Editor      │                                              │
│   .yal / .yalp /     │   Contenido del tab activo                  │
│   texto prueba       │   (tabla / grafo SVG / lista de estados)     │
│                      │                                              │
├──────────────────────┴──────────────────────────────────────────────┤
│  ParseConsole.jsx                                                    │
│  > [c c d c d              ]  [Parsear]  [◀ Paso]  [▶ Paso]        │
│  Estado 0 | token 'c' → Shift → I2                                  │
│  Estado 2 | token 'd' → Shift → I3 → Reduce(C→d) → GOTO I5         │
│  ✓ ACEPTADO                                                          │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Estado global (Zustand)

```js
{
  // Archivos
  yalContent: "",
  yalpContent: "",
  testInput: "",

  // Resultados léxico
  lexer: {
    tokens: [],            // [{ kind, lexeme, line, col }]
    dotNfa: "",            // string DOT → viz.js
    dotDfa: "",            // string DOT → viz.js
    transitionTable: null, // { delta[][], accept[], alphabet[] }
    generatedCode: "",     // lexer.rs generado
    errors: [],
  },

  // Resultados parser
  parser: {
    productions: [],       // gramática parseada
    firstSets: {},         // { NT: [terminals] }
    followSets: {},
    states: [],            // [{ id, items[], origin }]
    actionTable: {},       // { "(state,terminal)": acción }
    gotoTable: {},         // { "(state,NT)": estado }
    conflicts: [],
  },

  // Parseo paso a paso
  trace: {
    steps: [],             // [{ stack, token, action, description }]
    currentStep: 0,
  },

  // UI
  activeFile: "yal",       // "yal" | "yalp" | "test"
  activeResultTab: "states",
}
```

---

## Flujo de datos

```
CodeEditor
    │ onChange
    ▼
appStore (yalContent / yalpContent)
    │ [▶ Run]
    ▼
useLexer / useParser
    │ invoke("compilar_yal",  { content })
    │ invoke("compilar_yalp", { content })
    ▼
appStore actualizado
    │
    ├──► AutomataGraph      dotDfa → viz.js → SVG
    ├──► TransitionTable    delta[][] → grid
    ├──► TokenStream        tokens[]
    ├──► AutomataStates     states[]
    ├──► ActionGotoTable    actionTable + paso actual → celda resaltada
    └──► StepTrace          trace.steps[] → lista de pasos
```

---

## Orden de implementación

El orden que da valor visual más rápido:

1. **`CodeEditor`** — Monaco con tabs .yal / .yalp, corazón del IDE
2. **`ActionGotoTable`** — tabla ACTION/GOTO con celda activa resaltada
3. **`AutomataStates`** — lista de estados LR(1) con sus ítems, clickeable
4. **`AutomataGraph`** — viz.js con el `.dot` que ya produce el backend
5. **`StepTrace`** — consola paso a paso con botones ◀ / ▶
