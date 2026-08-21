# Referencia de funciones — qué ya existe

Índice de todo lo público (y los helpers privados reutilizables) para **no
reescribir código que ya está**. Buscar aquí antes de escribir una función
nueva. Formato: `archivo:línea` → firma → para qué sirve.

- [Léxico](#léxico-srclexico)
- [Sintáctico](#sintáctico-srcsintactico)
- [Semántico](#semántico-srcsemantico)
- [Capa API](#capa-api-srcapi)
- [Binarios](#binarios-srcbin)
- [Frontend](#frontend-frontendide)
- [Duplicación conocida](#duplicación-conocida)

---

## Léxico (`src/lexico/`)

### spec — parseo del `.yal`

| Ubicación | Firma | Qué hace |
|---|---|---|
| `spec/parser.rs:11` | `parse_yalex(input: &str) -> Result<SpecIR, LexerGenError>` | Parsea un `.yal` completo: header, `let`, `rule`, trailer |
| `spec/parser.rs:174` | `split_rule_pattern_action(line) -> Option<(String, String)>` *(priv)* | Separa `patrón { acción }` de una línea de regla |
| `spec/expand.rs:22` | `expand_definitions(spec: &SpecIR) -> Vec<ExpandedRule>` | Sustituye macros `let` dentro de los patrones |
| `spec/expand.rs:44` | `expand_string(input, defs) -> String` *(priv)* | Sustitución textual de una macro |

Tipos: `SpecIR` (`spec/ast.rs:6`), `Definition` (`:15`), `Rule` (`:22`), `ExpandedRule` (`spec/expand.rs:15`).

### regex — regex → AST

| Ubicación | Firma | Qué hace |
|---|---|---|
| `regex/parser.rs:8` | `parse_regex(input: &str) -> Result<RegexAst, LexerGenError>` | Parser recursivo-descendente de la regex a AST |
| `regex/ast.rs:8` | `enum RegexAst` | Nodos del AST (concat, unión, kleene, char, clase…) |

### automata — Thompson, subconjuntos, minimización

| Ubicación | Firma | Qué hace |
|---|---|---|
| `automata/nfa.rs:176` | `build_nfa_from_ast(ast, id_counter) -> Nfa` | Construcción de Thompson desde el AST de regex |
| `automata/nfa.rs:330` | `combine_nfas(nfas, id_counter) -> Nfa` | Une todos los NFA de regla en un "super-NFA" |
| `automata/nfa.rs:62` | `Nfa::add_transition(from, to, trans)` | Agrega transición (char o ε) |
| `automata/nfa.rs:90` | `tokenize_class_atoms(content)` *(priv)* | Tokeniza el contenido de `[...]` |
| `automata/nfa.rs:131` | `expand_char_class(content) -> (bool, Vec<char>)` *(priv)* | Expande rangos `a-z`; el bool indica negación |
| `automata/subset.rs:11` | `epsilon_closure(nfa, start_states) -> BTreeSet<usize>` | Cerradura-ε |
| `automata/subset.rs:32` | `move_to(nfa, current_states, c) -> BTreeSet<usize>` | Función `move` del subset construction |
| `automata/subset.rs:48` | `build_dfa_from_nfa(nfa: &Nfa) -> Dfa` | Construcción de subconjuntos NFA→DFA |
| `automata/minimize.rs:7` | `minimize_dfa(dfa: &Dfa) -> Dfa` | Minimización por particiones (Hopcroft/Moore) |

Tipos: `Nfa`/`State`/`Transition` (`automata/nfa.rs:38,15,9`), `Dfa`/`DfaState` (`automata/dfa.rs:17,5`).

### table — tabla de transiciones

| Ubicación | Firma | Qué hace |
|---|---|---|
| `table/transition_table.rs:96` | `build(dfa: &Dfa) -> TransitionTable` | Exporta el DFA a tabla plana |
| `table/transition_table.rs:21` | `TransitionTable::next(state, c) -> i32` | Transición (−1 = muerto) |
| `table/transition_table.rs:28` | `TransitionTable::is_accepting(state) -> bool` | ¿estado de aceptación? |
| `table/transition_table.rs:32` | `TransitionTable::token_at(state) -> Option<&str>` | Token asociado al estado |
| `table/transition_table.rs:45` | `kind_from_action(action: &str) -> String` | Extrae el nombre del token de `{ return X }` |

### runtime — simulador e INDENT/DEDENT

| Ubicación | Firma | Qué hace |
|---|---|---|
| `runtime/simulator.rs:32` | `Simulator::new(table, input)` | Crea el simulador sobre una fuente |
| `runtime/simulator.rs:43` | `Simulator::next_token() -> LexResult` | Un token con maximal munch (mantiene línea/columna) |
| `runtime/simulator.rs:169` | `Simulator::tokenize() -> (Vec<Token>, Vec<String>)` | Tokeniza todo; el segundo vector son errores léxicos |
| `runtime/indent.rs:30` | `is_indent_sensitive(grammar_tokens) -> bool` | Detecta si la gramática usa INDENT/DEDENT |
| `runtime/indent.rs:63` | `synthesize(...)` | Inserta tokens INDENT/DEDENT/NEWLINE sintéticos |

Tipos: `Token` (`:7`), `LexResult` (`:17`).

### codegen y graph

| Ubicación | Firma | Qué hace |
|---|---|---|
| `codegen/rust_codegen.rs:44` | `emit_string(...) -> String` | Genera el fuente del lexer standalone como `String` |
| `codegen/rust_codegen.rs:258` | `emit_file(...)` | Igual pero escribe a disco (`generated/`) |
| `codegen/rust_codegen.rs:174` | `emit_indent_support(code, ignored_kinds)` *(priv)* | Inyecta el bloque INDENT/DEDENT en el código emitido |
| `codegen/rust_codegen.rs:10` | `escape_rust_string(s)` *(priv)* | Escapa un literal Rust |
| `graph/dot.rs:8` | `write_ast_dot(path, root) -> io::Result<()>` | Exporta el AST de regex a Graphviz |
| `graph/dot.rs:52` | `write_dfa_dot(path, dfa) -> io::Result<()>` | Exporta el DFA a Graphviz |

Opciones: `CodegenOptions` (`codegen/rust_codegen.rs:29`).

---

## Sintáctico (`src/sintactico/`)

### gramatica — `.yalp` → `Grammar`, FIRST/FOLLOW

| Ubicación | Firma | Qué hace |
|---|---|---|
| `gramatica/grammar.rs:86` | `Grammar::parse_from_file(path)` | Parsea `.yalp` desde archivo |
| `gramatica/grammar.rs:105` | `Grammar::parse_for_lr(path)` | Idem, preparado para LR (agrega `S'`) |
| `gramatica/grammar.rs:114` | `Grammar::parse_for_lr_from_str(raw)` | **Usar este desde la API** — LR desde string |
| `gramatica/grammar.rs:122` | `Grammar::parse_for_ll1_from_str(raw)` | LL(1) desde string (aplica desambiguación) |
| `gramatica/grammar.rs:303` | `Grammar::eliminate_ambiguity()` | Factorización izquierda / desambiguación |
| `gramatica/grammar.rs:481` | `Grammar::detect_left_recursion()` | Detecta recursión izquierda (bloquea LL(1)) |
| `gramatica/grammar.rs:647` | `Grammar::validate()` | No-terminales sin producción, símbolos inalcanzables, etc. |
| `gramatica/grammar.rs:33` | `body_to_string(body)` | Formatea un cuerpo de producción |
| `gramatica/grammar.rs:45` | `dotted_body_to_string(body, dot_pos)` | Formatea con el punto del ítem LR — **reusar para mostrar ítems** |
| `gramatica/first.rs:13` | `calculate_first(grammar) -> FirstSets` | Conjuntos FIRST |
| `gramatica/first.rs:100` | `first_of_sequence(seq, first_sets) -> HashSet<String>` | FIRST de una secuencia de símbolos |
| `gramatica/follow.rs:13` | `calculate_follow(grammar, first_sets) -> FollowSets` | Conjuntos FOLLOW |

Tipos: `Symbol` (`grammar.rs:16`), `Production` (`:66`), `Grammar` (`:73`), `Associativity` (`:8`), `FirstSets`/`FollowSets` (alias de `HashMap<String, HashSet<String>>`).

### automatas — LR(0) / LR(1) / LALR

| Ubicación | Firma | Qué hace |
|---|---|---|
| `automatas/lr0.rs:28` | `LR0Automaton::closure(items, grammar)` | Cerradura de ítems LR(0) |
| `automatas/lr0.rs:67` | `LR0Automaton::goto(items, symbol, grammar)` | GOTO LR(0) |
| `automatas/lr0.rs:82` | `LR0Automaton::build(grammar) -> Self` | Colección canónica LR(0) (base de SLR) |
| `automatas/lr1.rs:69` | `LR1Automaton::closure(items, grammar, first_sets)` | Cerradura LR(1) con lookaheads |
| `automatas/lr1.rs:128` | `LR1Automaton::goto(...)` | GOTO LR(1) |
| `automatas/lr1.rs:155` | `LR1Automaton::build(grammar, first_sets) -> Self` | Colección canónica LR(1) |
| `automatas/lr1.rs:25` | `LR1Item::is_reduce_item() -> bool` | ¿el punto está al final? |
| `automatas/lr1.rs:30` | `LR1Item::display() -> String` | Ítem legible `A → α·β, {la}` |
| `automatas/lalr.rs:38` | `merge_by_core(lr1) -> LALRAutomaton` | Fusiona estados LR(1) con mismo core → LALR(1) |
| `automatas/lalr.rs:8` | `item_core(item)` *(priv)* | Core de un ítem (clave de fusión) |

Tipos: `LR0Item`/`State`/`LR0Automaton`, `LR1Item`/`LR1State`/`LR1Automaton`, `LALRItem`/`LALRState`/`LALRAutomaton`.

### tablas — ACTION/GOTO y conflictos

| Ubicación | Firma | Qué hace |
|---|---|---|
| `tablas.rs:77` | `LRTable::build_from_lalr(automaton, grammar)` | Tabla LALR(1) |
| `tablas.rs:133` | `LRTable::build_from_slr(automaton, grammar, follow)` | Tabla SLR(1) |
| `tablas.rs:194` | `LRTable::expected_tokens(state) -> Vec<String>` | Tokens válidos en un estado — **para mensajes de error** |
| `tablas.rs:204` | `LRTable::print_table(grammar)` | Imprime la tabla en consola |
| `tablas.rs:274` | `format_expected_tokens(tokens) -> String` | Formatea "se esperaba X, Y o Z" |
| `tablas.rs:337` | `insert_action(...)` | Inserta una acción resolviendo/registrando conflictos |
| `tablas.rs:442` | `print_productions(grammar)` | Lista numerada de producciones |
| `tablas.rs:37` | `Conflict::describe() -> String` | Descripción legible de un conflicto |
| `tablas.rs:291` | `build_prec_map(grammar)` *(priv)* | Mapa de precedencias por terminal |
| `tablas.rs:305` | `resolve_shift_reduce(...)` *(priv)* | Resolución shift/reduce por precedencia y asociatividad |
| `tablas.rs:412` | `build_production_index(grammar)` *(priv)* | Índice `(head, body) → nº de producción` |
| `tablas.rs:428` | `production_number(grammar, head, body)` *(priv)* | Nº de producción (usa el índice) |

Tipos: `Action` (`:14`), `Conflict` (`:21`), `LRTable` (`:56`), `PrecInfo` (`:286`).

### runtime — parsers y árbol

| Ubicación | Firma | Qué hace |
|---|---|---|
| `runtime/parser_lr.rs:25` | `LRParser::new(table)` | Parser LR dirigido por tabla |
| `runtime/parser_lr.rs:31` | `LRParser::parse(tokens) -> Result<Vec<ParseStep>, String>` | Parseo con traza |
| `runtime/parser_lr.rs:93` | `LRParser::parse_tree(tokens) -> Result<ParseNode, String>` | Parseo que devuelve el árbol |
| `runtime/parser_lr.rs:163` | `LRParser::parse_recovering(...)` | Parseo con recuperación de errores (modo pánico) |
| `runtime/parser_lr.rs:172` | `LRParser::parse_recovering_with_pos(...)` | Idem con línea/columna — **el que usa la API** |
| `runtime/parser_lr.rs:321` | `print_trace(trace)` | Imprime la traza en consola |
| `runtime/parser_lr.rs:314` | `format_error(state, token, table)` *(priv)* | Mensaje "token inesperado, se esperaba…" |
| `runtime/ll1.rs:25` | `LL1Parser::build(grammar, first, follow)` | Construye la tabla M[NT,T]; falla si hay conflicto |
| `runtime/ll1.rs:107` | `LL1Parser::parse(tokens) -> Result<(), String>` | Reconocimiento LL(1) |
| `runtime/ll1.rs:170` | `LL1Parser::parse_tree(tokens) -> Result<ParseNode, String>` | Árbol LL(1) |
| `runtime/ll1.rs:296` | `LL1Parser::parse_with_trace(tokens) -> Vec<LL1TraceStep>` | Traza paso a paso para el IDE |
| `runtime/parse_tree.rs:30` | `ParseToken::from_kinds(kinds) -> Vec<ParseToken>` | Convierte kinds sueltos a tokens |
| `runtime/parse_tree.rs:38` | `ParseNode::leaf(token)` / `:48` `epsilon_leaf()` / `:58` `internal(symbol, children)` | Constructores del árbol |
| `runtime/parse_tree.rs:64` | `print_ascii(root)` | Árbol en ASCII para consola |
| `runtime/parse_tree.rs:88` | `to_dot(root) -> String` | Árbol a Graphviz DOT |

Tipos: `ParseStep` (`parser_lr.rs:7`), `ParseErrorDetail` (`:14`), `LL1Table`/`LL1TraceStep` (`ll1.rs:16,287`), `ParseNode`/`ParseToken` (`parse_tree.rs:6,20`).

---

## Semántico (`src/semantico/`) — Fase 15, en progreso

| Ubicación | Firma | Qué hace |
|---|---|---|
| `analyzer/mod.rs:20` | `analyze(tree, spec) -> AnalysisResult` | Walker genérico sobre `ParseNode` dirigido por `SemanticSpec` |
| `analyzer/mod.rs:43` | `walk(node, spec, table, errors)` *(priv)* | Recorrido recursivo; aplica reglas de declaración y scope |
| `analyzer/mod.rs:32` | `find_identifier_child(...)` *(priv)* | Busca la hoja identificador bajo una producción |
| `symbols/mod.rs:141` | `SymbolTable::new()` | Tabla con scope global |
| `symbols/mod.rs:145` | `enter_scope(kind)` / `:149` `enter_scope_named(kind, label)` | Abre un entorno |
| `symbols/mod.rs:157` | `exit_scope() -> Result<Scope, SemanticError>` | Cierra el entorno (error si es el global) |
| `symbols/mod.rs:165` | `declare(...)` | Declara un símbolo; rechaza redeclaración en el mismo scope |
| `symbols/mod.rs:200` | `declare_typed(...)` | Declara con tipo/mutabilidad/firma |
| `symbols/mod.rs:238` | `assign(...)` | Valida asignación (const, tipos, coerción) |
| `symbols/mod.rs:271` | `lookup(name)` / `:278` `lookup_mut` / `:284` `lookup_or_err` | Búsqueda innermost-first |
| `symbols/mod.rs:292` | `depth()` / `:296` `current_scope_kind()` | Estado de la pila |
| `symbols/mod.rs:303` | `dump() -> String` | Volcado legible de todos los scopes activos |
| `scopes/mod.rs:87` | `ScopeStack::new()` / `:91` `enter` / `:99` `exit` | Pila de entornos |
| `scopes/mod.rs:106` | `current()` / `:110` `current_mut()` / `:114` `depth()` | Acceso al tope |
| `scopes/mod.rs:120,125,130` | `iter_innermost_first[_mut]`, `iter_outermost_first` | Iteradores de resolución |
| `scopes/mod.rs:49` | `Scope::get_own` / `:54` `get_own_mut` / `:58` `contains_own` / `:64` `symbols` | Acceso a un solo scope |
| `types/mod.rs:185` | `CompatibilityTable::arithmetic(...)` | Tipo resultante de un operador aritmético |
| `types/mod.rs:207` | `CompatibilityTable::assignment(expected, found)` | ¿asignación válida? qué coerción |
| `types/mod.rs:229` | `resolve_arithmetic(...)` / `:238` `resolve_assignment(...)` | Atajos libres sobre la tabla |

Tipos: `AnalysisResult` (`analyzer:11`), `SemanticSpec`/`DeclarationRule`/`ScopeRule` (`spec/mod.rs:9,19,49`), `Symbol`/`SymbolKind`/`Signature`/`StorageInfo`/`SemanticError`/`SymbolTable` (`symbols/mod.rs:61,20,37,48,84,136`), `Scope`/`ScopeKind`/`ScopeStack`/`PopGlobalScope` (`scopes/mod.rs:29,18,82,78`), `Type`/`ArithmeticOperator`/`Coercion`/`ArithmeticResolution`/`TypeError` (`types/mod.rs:12,39,58,66,73`).

---

## Capa API (`src/api/`)

Compartida por `src/bin/api.rs` y por los tests de integración. **Antes de
escribir un pipeline nuevo, reusar estas cuatro.**

| Ubicación | Firma | Qué hace |
|---|---|---|
| `sintactico.rs:21` | `build_compile_response(content, mode) -> Result<CompileResponse, String>` | `.yalp` → estados, ACTION/GOTO, FIRST/FOLLOW, producciones, problemas, DOT del LR(0) |
| `sintactico.rs:82` | `build_parse_response(content, tokens, mode) -> Result<ParseResponse, String>` | Parsea una lista de kinds y devuelve la traza |
| `pipeline.rs:14` | `build_pipeline_response(yal, yalp, source, mode) -> Result<ParseResponse, String>` | End-to-end: `.yal` + `.yalp` + fuente → tokens + traza + problemas |
| `codegen.rs:16` | `build_codegen_response(yal, yalp) -> Result<CodegenResponse, String>` | Genera el fuente del lexer standalone |
| `lexico.rs:17` | `build_lexer_artifacts(yal) -> Result<(SpecIR, Vec<ExpandedRule>, TransitionTable), String>` | **Pipeline léxico completo en una llamada** — usar en vez de reencadenar parse→expand→NFA→DFA→tabla |
| `lexico.rs:41` | `build_lexer_table_from_str(yal)` *(crate)* | Igual pero solo la tabla |
| `dot.rs:8` | `lr0_to_dot(automaton) -> String` *(crate)* | Autómata LR(0) a Graphviz (lo consume viz.js en el IDE) |

Helpers privados de `api/sintactico.rs` (reutilizables dentro del módulo):

| Línea | Función | Qué hace |
|---|---|---|
| `153` | `build_compile_ll1(content)` | Rama LL(1) de compile |
| `359` | `parse_with_trace_lr(table, tokens)` | Traza LR serializada a JSON |
| `457` / `470` | `lalr_states_to_data` / `lr0_states_to_data` | Estados → `StateData` |
| `484` | `format_lalr_item(it)` | Ítem LALR legible |
| `515` / `530` | `action_table_to_map` / `goto_table_to_map` | Tablas → mapas JSON |
| `538` | `sets_to_sorted_vecs(...)` | FIRST/FOLLOW → vectores ordenados (salida determinista) |
| `555` | `grammar_to_prods(grammar)` | Producciones → `ProdData` |
| `576` | `build_problems(conflicts, state_count, mode)` | Conflictos → `ProblemData` del panel PROBLEMS |
| `603` | `levenshtein(a, b)` | Distancia de edición — **ya existe, no reescribir** |
| `627` | `suggest_similar_token(lexeme, tokens)` | "¿quisiste decir X?" |
| `650` | `is_identifier_like(s)` | Heurística de identificador |

Tipos de respuesta JSON (`api/mod.rs`): `StateData:29`, `ProdData:35`, `ProblemData:42`, `ParseResponse:50`, `CodegenResponse:59`, `CompileResponse:67`.

---

## Binarios (`src/bin/`)

### `api.rs` — servidor Axum (:8080)

| Línea | Ruta | Handler |
|---|---|---|
| `164` | `GET /health` | `health` (`:105`) |
| `165` | `POST /api/parser/compile` | `compile_parser` (`:109`) |
| `166` | `POST /api/parser/parse` | `parse_tokens` (`:117`) |
| `167` | `POST /api/pipeline` | `run_pipeline` (`:125`) |
| `168` | `POST /api/codegen` | `run_codegen` (`:133`) |
| `169` | `GET /api/workspace` | `workspace_list` (`:67`) |
| `170` | `GET\|PUT /api/workspace/:name` | `workspace_read` (`:83`) / `workspace_write` (`:93`) |

Helpers: `default_mode():51` (`"lalr"`), `workspace_dir():58`, `sanitize_filename(name):60` (**anti path-traversal — usarlo en cualquier ruta nueva que reciba un nombre de archivo**).

### Otros binarios

| Archivo | Qué es |
|---|---|
| `test_pipeline.rs` | CLI end-to-end `.yal` + `.yalp` + fuente → árbol. Helpers: `build_lexer_table(yal_path):190`, `normalize_kind(kind):223`, `is_ignored(kind, grammar):229` |
| `test_lalr.rs` | REPL de consola LALR(1)/LR(1) |
| `test_ll1.rs` | REPL de consola LL(1) |
| `../main.rs` | CLI del generador de lexers standalone → escribe `generated/` |

Errores: `LexerGenError` (`src/error.rs:6`).

---

## Frontend (`frontend/IDE/`)

Sin bundler: el HTML carga React/Babel/viz.js por CDN y compila los `.jsx` en
el navegador. Al editar un `.jsx` hay que subir el `?v=N` del `<script>`
correspondiente en `IDE Analizador Sintactico.html:535-536`.

### `data.jsx` — estado global

Define `window.IDE_DATA` (`:114`), aliasado como `D` en `app.jsx:3`. Contiene
mock data (`FILES`, `STATES`, `ACTION`, `GOTO`, `TERMINALS`, `NONTERMINALS`,
`FIRST`, `FOLLOW`, `PRODS`, `TRACE`, `TOKENS:100`, `PROBLEMS:109`) que el
backend sobreescribe en RUN/PARSEAR, más los campos que **solo** se llenan
desde la API: `PARSE_ACCEPTED`, `PARSE_ERROR`, `GEN_CODE`, `LR0_DOT`,
`PARSE_TREE_DOT`. También `YAL_RAW`/`YALP_RAW`/`TEST_RAW` como contenido inicial.

### `app.jsx` — un componente por panel

| Línea | Componente / función | Qué hace |
|---|---|---|
| `5` | `const API` | Base URL del backend — hardcodeada a `http://localhost:8080` |
| `9` | `FileTree` | Árbol de archivos + botones de carga |
| `46` | `escHtml(s)` | Escapa HTML antes de inyectar highlight |
| `48` | `HL_RULES` | Reglas de syntax highlighting por lenguaje |
| `69` | `tokenize(text, lang)` | Tokenizador del editor (solo para colorear) |
| `93` | `Editor` | Editor editable con highlight y subrayado de errores |
| `195` | `GrammarView` | Producciones numeradas |
| `228` | `FirstFollow({which})` | Tabla FIRST o FOLLOW |
| `252` | `StatesView` | Lista de estados con sus ítems |
| `317` | `LL1TableView` | Tabla M[NT,T] |
| `363` | `ActionGotoTable` | Tabla ACTION/GOTO con la fila del paso actual resaltada |
| `431` | `TokensView` | Tokens del lexer (kind, lexema, línea, columna) |
| `455` | `LR0Graph` | Renderiza `D.LR0_DOT` con viz.js |
| `497` | `GeneratedCode` | Muestra `D.GEN_CODE` |
| `532` | `ProblemsList` | Panel PROBLEMS desde `D.PROBLEMS` |
| `574` / `598` | `_buildTreeLR` / `_buildTreeLL1` | Reconstruyen el árbol desde la traza |
| `628` | `buildParseTree(trace, mode)` | Dispatcher de los dos anteriores |
| `634` | `buildTreeDot(root)` | Árbol → DOT (con `visit` interno en `:644`) |
| `662` | `ParseTreeView` | Renderiza el árbol con viz.js |
| `726` | `StackView` | Pila del parser en el paso actual |
| `770` | `ParseConsole` | Traza paso a paso, controles de navegación |
| `1015` | `ResultsPanel` | Contenedor con pestañas de todo lo anterior |
| `1066` | `MODE_LABELS` | `lalr`/`slr`/`ll1` → etiqueta visible |
| `1068` | `Header` | Botones RUN / SAVE / selector de modo |
| `1115` | `StatusBar` | Barra inferior |
| `1138` | `App` | Estado global y llamadas al backend: `/api/workspace` (`:1169`, `:1182`, `:1218`, `:1235`), `/api/parser/compile` (`:1258`), `/api/codegen` (`:1288`), `/api/pipeline` (`:1355`), `/api/parser/parse` (`:1378`) |

---

## Duplicación conocida

Cosas que ya están duplicadas — no agregar una copia más:

1. **Pipeline `.yal` → tabla**: existe en `main.rs`, `bin/test_pipeline.rs:190`
   y `api/lexico.rs`. Para código nuevo usar `api::build_lexer_artifacts`
   (o `build_lexer_table_from_str` si solo se necesita la tabla).
2. **Escape de etiquetas DOT**: idéntico en `lexico/graph/dot.rs:47`
   (`escape_dot_label`) y `sintactico/runtime/parse_tree.rs:119` (`escape_dot`);
   `api/dot.rs` lo hace inline en `:23` y `:43`. Si se toca uno, considerar unificar.
3. **Emisores de DOT**: tres — AST/DFA (`lexico/graph/dot.rs`), árbol de
   derivación (`parse_tree.rs:88`), autómata LR(0) (`api/dot.rs:8`). Son grafos
   distintos, la separación es correcta; solo el escape se repite.
4. **Normalización de kinds**: `table/transition_table.rs:45` (`kind_from_action`)
   y `bin/test_pipeline.rs:223` (`normalize_kind`).
