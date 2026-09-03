// Compila una gramática (.yalp) a tablas de parseo y las ejecuta contra un
// stream de tokens. Cubre los tres modos (LALR/SLR vía LRTable, LL(1) vía
// LL1Parser) y la conversión de sus resultados a las formas JSON-friendly
// que consume el frontend.
use crate::sintactico::gramatica::first::calculate_first;
use crate::sintactico::gramatica::follow::calculate_follow;
use crate::sintactico::gramatica::grammar::{body_to_string, Grammar, Symbol};
use crate::sintactico::automatas::lalr::{merge_by_core, LALRItem};
use crate::sintactico::runtime::ll1::LL1Parser;
use crate::sintactico::automatas::lr0::{LR0Automaton, LR0Item};
use crate::sintactico::automatas::lr1::LR1Automaton;
use crate::sintactico::runtime::parse_tree::{ParseNode, ParseToken};
use crate::sintactico::runtime::driver::{self, DriveError, ErrorCause, OnError, ParseObserver};
use crate::sintactico::runtime::parser_lr::{tokens_from_kinds, LRParser};
use crate::sintactico::tablas::{format_expected_tokens, Action, Conflict, LRTable};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use super::dot::lr0_to_dot;
use super::{CompileResponse, ParseResponse, ProblemData, ProdData, StateData};

pub fn build_compile_response(content: &str, mode: &str) -> Result<CompileResponse, String> {
    if mode == "ll1" {
        return build_compile_ll1(content);
    }

    let grammar = Grammar::parse_for_lr_from_str(content)?;
    let first_sets = calculate_first(&grammar);
    let follow_sets = calculate_follow(&grammar, &first_sets);

    let (states_data, table, lr0_dot) = if mode == "slr" {
        let lr0 = LR0Automaton::build(&grammar);
        let dot = lr0_to_dot(&lr0);
        let data = lr0_states_to_data(&lr0);
        let table = LRTable::build_from_slr(&lr0, &grammar, &follow_sets);
        (data, table, dot)
    } else {
        let lr0 = LR0Automaton::build(&grammar);
        let dot = lr0_to_dot(&lr0);
        let lr1 = LR1Automaton::build(&grammar, &first_sets);
        let lalr = merge_by_core(lr1);
        let data = lalr_states_to_data(&lalr.states);
        let table = LRTable::build_from_lalr(&lalr, &grammar);
        (data, table, dot)
    };

    let action_map = action_table_to_map(&table, &grammar);
    let goto_map = goto_table_to_map(&table);

    let mut terminals: Vec<String> = grammar.tokens.iter().cloned().collect();
    terminals.push("$".to_string());
    terminals.sort();

    let mut seen = std::collections::HashSet::new();
    let non_terminals: Vec<String> = grammar
        .productions
        .iter()
        .filter(|p| seen.insert(p.head.clone()))
        .map(|p| p.head.clone())
        .collect();

    let first_map = sets_to_sorted_vecs(&first_sets);
    let follow_map = sets_to_sorted_vecs(&follow_sets);
    let prods = grammar_to_prods(&grammar);
    let state_count = states_data.len();
    let problems = build_problems(&table.conflicts, state_count, mode);

    Ok(CompileResponse {
        states: states_data,
        action: action_map,
        goto: goto_map,
        terminals,
        non_terminals,
        first: first_map,
        follow: follow_map,
        prods,
        problems,
        start_symbol: grammar.start_symbol.clone(),
        lr0_dot,
    })
}

/// Construye la tabla ACTION/GOTO para `mode`. Punto único: la misma cadena
/// (FIRST → autómata → tabla) estaba escrita tal cual en `build_parse_response`,
/// `build_real_parse_tree` y `collect_lr_syntax_errors`, y como cada una la
/// rehacía desde cero, UNA petición del IDE construía el autómata dos veces.
pub(crate) fn build_lr_table(grammar: &Grammar, mode: &str) -> LRTable {
    let first_sets = calculate_first(grammar);
    if mode == "slr" {
        let follow_sets = calculate_follow(grammar, &first_sets);
        let lr0 = LR0Automaton::build(grammar);
        LRTable::build_from_slr(&lr0, grammar, &follow_sets)
    } else {
        let lr1 = LR1Automaton::build(grammar, &first_sets);
        let lalr = merge_by_core(lr1);
        LRTable::build_from_lalr(&lalr, grammar)
    }
}

/// Convierte el `token_map` del pipeline a los tokens que consume el parser.
pub(crate) fn to_parse_tokens(token_map: &[(String, String, usize, usize)]) -> Vec<ParseToken> {
    token_map
        .iter()
        .map(|(k, lx, line, col)| ParseToken { kind: k.clone(), lexeme: lx.clone(), line: *line, col: *col })
        .collect()
}

/// La parte LR de `build_parse_response`, con la tabla YA construida.
pub(crate) fn build_parse_response_with(table: &LRTable, tokens: Vec<String>) -> ParseResponse {
    let trace = parse_with_trace_lr(table, tokens);
    let accepted = trace
        .last()
        .and_then(|s| s["action"].as_str())
        .map(|a| a == "acc")
        .unwrap_or(false);
    let error = if !accepted {
        trace.last().and_then(|s| s["desc"].as_str()).map(String::from)
    } else {
        None
    };
    ParseResponse { trace, accepted, error, ..Default::default() }
}

/// El árbol real, con la tabla YA construida. Solo LR: el camino LL(1) rearma
/// su propia gramática porque la factoriza y renombra producciones.
pub(crate) fn build_real_parse_tree_with(
    table: &LRTable,
    token_map: &[(String, String, usize, usize)],
) -> Option<ParseNode> {
    LRParser::new(table).parse_tree(to_parse_tokens(token_map)).ok()
}

/// Los errores sintácticos en modo pánico, con la tabla YA construida.
pub(crate) fn collect_lr_syntax_errors_with(
    table: &LRTable,
    grammar: &Grammar,
    token_map: &[(String, String, usize, usize)],
) -> Vec<(String, String, usize, usize, String)> {
    // Cualquier token declarado sirve como punto de sincronización: en cuanto se
    // descarta la entrada hasta encontrar UNO que el estado recuperado pueda
    // aceptar, se reintenta desde ahí — una heurística genérica razonable sin
    // conocer de antemano la estructura de la gramática (p. ej. cuál token hace
    // de separador de sentencias).
    let sync: Vec<&str> = grammar.tokens.iter().map(|t| t.as_str()).collect();
    let (_, errors) = LRParser::new(table).parse_recovering_with_pos(to_parse_tokens(token_map), &sync);

    errors
        .into_iter()
        .map(|e| {
            let (kind, lexeme, line, col) = token_map
                .get(e.pos)
                .cloned()
                .or_else(|| token_map.last().cloned())
                .unwrap_or_else(|| (e.token.clone(), String::new(), 1, 1));
            (kind, lexeme, line, col, e.msg)
        })
        .collect()
}

pub fn build_parse_response(
    content: &str,
    tokens: Vec<String>,
    mode: &str,
) -> Result<ParseResponse, String> {
    if mode == "ll1" {
        let grammar = Grammar::parse_for_ll1_from_str(content)?;
        let first_sets = calculate_first(&grammar);
        let follow_sets = calculate_follow(&grammar, &first_sets);
        let parser = LL1Parser::build(&grammar, &first_sets, &follow_sets)?;

        let steps = parser.parse_with_trace(tokens);
        let accepted = steps.last().map(|s| s.action == "acc").unwrap_or(false);
        let error = if !accepted {
            steps.last().map(|s| s.desc.clone())
        } else {
            None
        };
        let trace: Vec<Value> = steps
            .into_iter()
            .map(|s| {
                json!({
                    "stack": s.stack,
                    "remaining": s.remaining,
                    "action": s.action,
                    "desc": s.desc,
                    "pos": s.pos,
                })
            })
            .collect();
        return Ok(ParseResponse {
            trace,
            accepted,
            error,
            ..Default::default()
        });
    }

    let grammar = Grammar::parse_for_lr_from_str(content)?;
    let table = build_lr_table(&grammar, mode);
    Ok(build_parse_response_with(&table, tokens))
}

fn build_compile_ll1(content: &str) -> Result<CompileResponse, String> {
    let grammar = Grammar::parse_for_ll1_from_str(content)?;
    let first_sets = calculate_first(&grammar);
    let follow_sets = calculate_follow(&grammar, &first_sets);
    let ll1 = LL1Parser::build(&grammar, &first_sets, &follow_sets)?;

    // Representar la tabla M[A,a] como pseudo-estados
    let mut states: Vec<StateData> = grammar
        .productions
        .iter()
        .enumerate()
        .map(|(id, prod)| {
            let nt = &prod.head;
            let mut items: Vec<String> = Vec::new();
            if let Some(row) = ll1.table.get(nt) {
                let mut pairs: Vec<(&String, &crate::sintactico::gramatica::grammar::Production)> =
                    row.iter().collect();
                pairs.sort_by_key(|(t, _)| t.as_str());
                for (terminal, production) in pairs {
                    let rhs: Vec<String> = production
                        .bodies
                        .iter()
                        .flat_map(|b| {
                            if b.is_empty() {
                                vec!["ε".to_string()]
                            } else {
                                b.iter().map(|s| s.to_string()).collect()
                            }
                        })
                        .collect();
                    items.push(format!("M[{}, {}] → {}", nt, terminal, rhs.join(" ")));
                }
            }
            StateData { id, items }
        })
        .collect();
    states.sort_by_key(|s| s.id);

    // ACTION como tabla LL1: id_produccion → terminal → "NT → RHS"
    let mut action_map: HashMap<String, HashMap<String, String>> = HashMap::new();
    for (id, prod) in grammar.productions.iter().enumerate() {
        let nt = &prod.head;
        if let Some(row) = ll1.table.get(nt) {
            for (terminal, production) in row {
                let rhs: Vec<String> = production
                    .bodies
                    .iter()
                    .flat_map(|b| {
                        if b.is_empty() {
                            vec!["ε".to_string()]
                        } else {
                            b.iter().map(|s| s.to_string()).collect()
                        }
                    })
                    .collect();
                action_map
                    .entry(id.to_string())
                    .or_default()
                    .insert(terminal.clone(), format!("{} → {}", nt, rhs.join(" ")));
            }
        }
    }

    let mut terminals: Vec<String> = grammar.tokens.iter().cloned().collect();
    terminals.push("$".to_string());
    terminals.sort();

    let mut seen = std::collections::HashSet::new();
    let non_terminals: Vec<String> = grammar
        .productions
        .iter()
        .filter(|p| seen.insert(p.head.clone()))
        .map(|p| p.head.clone())
        .collect();

    let nt_count = non_terminals.len();
    let problems = vec![ProblemData {
        level: "info".to_string(),
        code: "I100".to_string(),
        msg: format!("gramática es LL(1) sin conflictos · {} no-terminales", nt_count),
        loc: "parser.yalp".to_string(),
    }];

    let lr0_dot = Grammar::parse_for_lr_from_str(content)
        .map(|g| {
            let lr0 = LR0Automaton::build(&g);
            lr0_to_dot(&lr0)
        })
        .unwrap_or_default();

    Ok(CompileResponse {
        states,
        action: action_map,
        goto: HashMap::new(),
        terminals,
        non_terminals,
        first: sets_to_sorted_vecs(&first_sets),
        follow: sets_to_sorted_vecs(&follow_sets),
        prods: grammar_to_prods(&grammar),
        problems,
        start_symbol: grammar.start_symbol.clone(),
        lr0_dot,
    })
}

/// Modo pánico: recupera TODOS los errores de sintaxis del token_map en una sola
/// pasada, en vez de solo el primero (B2). Solo disponible para LALR/SLR — LL(1)
/// no tiene LRTable/LRParser, así que el llamador debe usar el reporte de un solo
/// error como fallback en ese caso (o si la recuperación no encuentra nada).
pub(crate) fn collect_lr_syntax_errors(
    yalp: &str,
    token_map: &[(String, String, usize, usize)],
    mode: &str,
) -> Vec<(String, String, usize, usize, String)> {
    if mode != "lalr" && mode != "slr" {
        return Vec::new();
    }
    let Ok(grammar) = Grammar::parse_for_lr_from_str(yalp) else { return Vec::new(); };
    let table = build_lr_table(&grammar, mode);
    collect_lr_syntax_errors_with(&table, &grammar, token_map)
}

/// Enriquece un error de sintaxis con una sugerencia de token por distancia de
/// Levenshtein (si el lexema se parece a algún token declarado) y lo empuja como
/// entrada de `problems`. Compartido entre el reporte de un solo error y el modo
/// pánico multi-error (B2) para que ambos caminos generen el mismo formato.
pub(crate) fn push_syntax_problem(
    lex_problems: &mut Vec<Value>,
    kind: &str,
    lexeme: &str,
    line: usize,
    col: usize,
    base_msg: &str,
    tokens: &HashSet<String>,
    source_name: &str,
) {
    let suggestion = suggest_similar_token(lexeme, tokens);
    if let Some(s) = suggestion {
        if is_identifier_like(lexeme) && s != kind {
            lex_problems.push(json!({
                "level": "err",
                "code": "L001",
                "msg": format!("Lexema posiblemente mal escrito: '{}' (¿quiso '{}'?)", lexeme, s),
                "loc": format!("{}:{}:{}", source_name, line, col),
                "line": line,
                "col": col,
                "token": kind,
            }));
        } else {
            let msg = format!("{} · lexema \"{}\" (¿quiso \"{}\"?)", base_msg, lexeme, s);
            lex_problems.push(json!({
                "level": "err",
                "code": "P001",
                "msg": msg,
                "loc": format!("{}:{}:{}", source_name, line, col),
                "line": line,
                "col": col,
                "token": kind,
            }));
        }
    } else {
        lex_problems.push(json!({
            "level": "err",
            "code": "P001",
            "msg": format!("{} · lexema \"{}\"", base_msg, lexeme),
            "loc": format!("{}:{}:{}", source_name, line, col),
            "line": line,
            "col": col,
            "token": kind,
        }));
    }
}

/// Construye el árbol de derivación REAL (no el trace JSON de
/// `parse_with_trace_lr`) para el mismo `token_map` ya usado para parsear —
/// usado por el pipeline para poblar `parse_tree_dot`/`symbol_table` en la
/// respuesta HTTP. Devuelve también la `Grammar` ya parseada (con las
/// directivas `%ident`/`%declare`/`%scope`, si las trae) para que el llamador
/// pueda decidir si corre análisis semántico. `None` si la gramática o el
/// árbol no se pueden construir — el llamador simplemente no llena esos campos.
pub(crate) fn build_real_parse_tree(
    yalp: &str,
    token_map: &[(String, String, usize, usize)],
    mode: &str,
) -> Option<(Grammar, ParseNode)> {
    let parse_tokens: Vec<ParseToken> = token_map
        .iter()
        .map(|(k, lx, line, col)| ParseToken { kind: k.clone(), lexeme: lx.clone(), line: *line, col: *col })
        .collect();

    if mode == "ll1" {
        let grammar = Grammar::parse_for_ll1_from_str(yalp).ok()?;
        let first_sets = calculate_first(&grammar);
        let follow_sets = calculate_follow(&grammar, &first_sets);
        let parser = LL1Parser::build(&grammar, &first_sets, &follow_sets).ok()?;
        let tree = parser.parse_tree(parse_tokens).ok()?;
        Some((grammar, tree))
    } else {
        let grammar = Grammar::parse_for_lr_from_str(yalp).ok()?;
        let table = build_lr_table(&grammar, mode);
        let tree = LRParser::new(&table).parse_tree(parse_tokens).ok()?;
        Some((grammar, tree))
    }
}

/// Traza paso a paso para el IDE. Es el CUARTO consumidor del motor
/// shift-reduce; como los otros tres, ya no reimplementa el bucle: lo obtiene
/// de `sintactico::runtime::driver` y solo aporta qué anotar en cada evento.
///
/// Lo que lo distingue de los demás es que necesita una FOTO de la pila y de
/// lo que queda por consumir ANTES de cada acción — de ahí el callback
/// `before_step` del observador.
struct JsonTraceObserver {
    trace: Vec<Value>,
    /// Pila de símbolos en paralelo a la de estados del driver, para poder
    /// mostrarlas intercaladas como hace el IDE.
    symbols: Vec<String>,
    /// Última foto tomada en `before_step`, que es la que acompaña a la
    /// entrada de traza de esta iteración.
    snapshot_stack: Vec<Value>,
    snapshot_remaining: Vec<Value>,
}

impl JsonTraceObserver {
    fn new() -> Self {
        JsonTraceObserver {
            trace: Vec::new(),
            symbols: Vec::new(),
            snapshot_stack: Vec::new(),
            snapshot_remaining: Vec::new(),
        }
    }

    fn push_entry(&mut self, action: &str, desc: String) {
        self.trace.push(json!({
            "stack": self.snapshot_stack.clone(),
            "remaining": self.snapshot_remaining.clone(),
            "action": action,
            "desc": desc,
        }));
    }
}

impl ParseObserver for JsonTraceObserver {
    fn before_step(&mut self, states: &[usize], remaining: &[ParseToken]) {
        self.snapshot_remaining = remaining.iter().map(|t| json!(t.kind)).collect();
        let mut stack_val: Vec<Value> = Vec::new();
        for (i, st) in states.iter().enumerate() {
            stack_val.push(json!(st));
            if i < self.symbols.len() {
                stack_val.push(json!(self.symbols[i]));
            }
        }
        self.snapshot_stack = stack_val;
    }

    fn on_shift(&mut self, next_state: usize, token: &ParseToken) {
        self.symbols.push(token.kind.clone());
        self.push_entry(
            &format!("s{}", next_state),
            format!("Shift '{}' → I{}", token.kind, next_state),
        );
    }

    fn on_reduce(&mut self, head: &str, body: &[Symbol], _goto: usize) {
        let body_str = body_to_string(body);
        for _ in 0..body.len() {
            self.symbols.pop();
        }
        self.symbols.push(head.to_string());
        self.push_entry("r", format!("{} → {}", head, body_str));
    }

    fn on_accept(&mut self) {
        self.push_entry("acc", "Cadena aceptada".to_string());
    }

    fn on_error(
        &mut self,
        _cause: ErrorCause,
        state: usize,
        _ip: usize,
        token: &ParseToken,
        table: &LRTable,
    ) -> OnError {
        let expected_str = format_expected_tokens(&table.expected_tokens(state));
        self.push_entry(
            "error",
            format!(
                "Error sintáctico en I{}, token '{}'. Esperado: {}",
                state, token.kind, expected_str
            ),
        );
        OnError::Abort
    }
}

fn parse_with_trace_lr(table: &LRTable, tokens: Vec<String>) -> Vec<Value> {
    let mut obs = JsonTraceObserver::new();
    let result = driver::drive(table, tokens_from_kinds(tokens), &[], &mut obs);

    // `Syntax` ya dejó su entrada en `on_error`; los otros dos cortes son
    // internos y el driver no avisa por callback, así que se anotan acá.
    match result {
        Err(DriveError::MissingGoto { top, head, .. }) => {
            obs.push_entry("error", DriveError::missing_goto_msg(top, &head));
        }
        Err(DriveError::InputExhausted) => {
            obs.trace.push(json!({
                "stack": Value::Null,
                "remaining": Value::Array(vec![]),
                "action": "error",
                "desc": DriveError::exhausted_msg(),
            }));
        }
        _ => {}
    }
    obs.trace
}

fn lalr_states_to_data(states: &[crate::sintactico::automatas::lalr::LALRState]) -> Vec<StateData> {
    let mut v: Vec<StateData> = states
        .iter()
        .map(|s| {
            let mut items: Vec<String> = s.items.iter().map(format_lalr_item).collect();
            items.sort();
            StateData { id: s.id, items }
        })
        .collect();
    v.sort_by_key(|s| s.id);
    v
}

fn lr0_states_to_data(automaton: &LR0Automaton) -> Vec<StateData> {
    let mut v: Vec<StateData> = automaton
        .states
        .iter()
        .map(|s| {
            let mut items: Vec<String> = s.items.iter().map(format_lr0_item).collect();
            items.sort();
            StateData { id: s.id, items }
        })
        .collect();
    v.sort_by_key(|s| s.id);
    v
}

fn format_lalr_item(it: &LALRItem) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, sym) in it.body.iter().enumerate() {
        if i == it.dot_pos {
            parts.push("•".to_string());
        }
        parts.push(sym.to_string());
    }
    if it.dot_pos == it.body.len() {
        parts.push("•".to_string());
    }
    let mut las: Vec<&str> = it.lookaheads.iter().map(|s| s.as_str()).collect();
    las.sort();
    format!("[{} -> {}, {{{}}}]", it.head, parts.join(" "), las.join(","))
}

/// También la usa `dot::lr0_to_dot` para etiquetar los nodos del grafo.
pub(crate) fn format_lr0_item(it: &LR0Item) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, sym) in it.body.iter().enumerate() {
        if i == it.dot_pos {
            parts.push("•".to_string());
        }
        parts.push(sym.to_string());
    }
    if it.dot_pos == it.body.len() {
        parts.push("•".to_string());
    }
    format!("[{} -> {}]", it.head, parts.join(" "))
}

fn action_table_to_map(table: &LRTable, _grammar: &Grammar) -> HashMap<String, HashMap<String, String>> {
    let mut map: HashMap<String, HashMap<String, String>> = HashMap::new();
    for ((state, terminal), action) in &table.action {
        let s = match action {
            Action::Shift(n) => format!("s{}", n),
            Action::Reduce { head, body } => format!("r({} → {})", head, body_to_string(body)),
            Action::Accept => "acc".to_string(),
        };
        map.entry(state.to_string())
            .or_default()
            .insert(terminal.clone(), s);
    }
    map
}

fn goto_table_to_map(table: &LRTable) -> HashMap<String, HashMap<String, usize>> {
    let mut map: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for ((state, nt), dest) in &table.goto {
        map.entry(state.to_string()).or_default().insert(nt.clone(), *dest);
    }
    map
}

fn sets_to_sorted_vecs(
    sets: &HashMap<String, std::collections::HashSet<String>>,
) -> HashMap<String, Vec<String>> {
    sets
        .iter()
        .map(|(k, v)| {
            let mut vals: Vec<String> = v
                .iter()
                .filter(|s| s.as_str() != "ε")
                .cloned()
                .collect();
            vals.sort();
            (k.clone(), vals)
        })
        .collect()
}

fn grammar_to_prods(grammar: &Grammar) -> Vec<ProdData> {
    let mut prods: Vec<ProdData> = Vec::new();
    let mut n = 1usize;
    for prod in &grammar.productions {
        for body in &prod.bodies {
            let rhs = if body.is_empty() {
                vec!["ε".to_string()]
            } else {
                body.iter().map(|s| s.to_string()).collect()
            };
            prods.push(ProdData {
                n,
                lhs: prod.head.clone(),
                rhs,
            });
            n += 1;
        }
    }
    prods
}

fn build_problems(conflicts: &[Conflict], state_count: usize, mode: &str) -> Vec<ProblemData> {
    if conflicts.is_empty() {
        vec![ProblemData {
            level: "info".to_string(),
            code: "I100".to_string(),
            msg: format!(
                "gramática {} sin conflictos · {} estados",
                mode.to_uppercase(),
                state_count
            ),
            loc: "parser.yalp".to_string(),
        }]
    } else {
        conflicts
            .iter()
            .enumerate()
            .map(|(i, c)| ProblemData {
                level: "warn".to_string(),
                code: format!("W{:03}", i + 1),
                msg: c.describe(),
                loc: "parser.yalp".to_string(),
            })
            .collect()
    }
}

/// Distancia de Levenshtein simple entre dos cadenas (por caracteres).
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let n = a_chars.len();
    let m = b_chars.len();
    if n == 0 { return m; }
    if m == 0 { return n; }
    let mut dp: Vec<Vec<usize>> = vec![vec![0; m + 1]; n + 1];
    for i in 0..=n { dp[i][0] = i; }
    for j in 0..=m { dp[0][j] = j; }
    for i in 0..n {
        for j in 0..m {
            let cost = if a_chars[i] == b_chars[j] { 0 } else { 1 };
            dp[i + 1][j + 1] = std::cmp::min(
                std::cmp::min(dp[i][j + 1] + 1, dp[i + 1][j] + 1),
                dp[i][j] + cost,
            );
        }
    }
    dp[n][m]
}

/// Busca un token similar en la lista `tokens` al `lexeme` dado.
/// Devuelve el nombre del token sugerido si la distancia es razonablemente baja.
fn suggest_similar_token(lexeme: &str, tokens: &HashSet<String>) -> Option<String> {
    if lexeme.is_empty() || tokens.is_empty() { return None; }
    let lex_low = lexeme.to_lowercase();
    let mut best: Option<String> = None;
    let mut best_d: usize = usize::MAX;
    for t in tokens.iter() {
        let t_low = t.to_lowercase();
        let d = levenshtein(&lex_low, &t_low);
        if d < best_d {
            best_d = d;
            best = Some(t.clone());
        }
    }
    // Umbral heurístico: distancia <= 2 o <= 1/3 del tamaño de la palabra
    if best_d <= 2 || best_d <= lex_low.len() / 3 {
        best
    } else {
        None
    }
}

/// Heurística simple para detectar si un lexema tiene pinta de identificador
/// (letras, dígitos y guiones bajos, no contiene espacios ni signos de puntuación).
fn is_identifier_like(s: &str) -> bool {
    if s.is_empty() { return false; }
    // Considerar identificadores que empiezan por letra o '_' y contienen solo [A-Za-z0-9_]
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !(first.is_alphabetic() || first == '_') { return false; }
    for c in chars {
        if !(c.is_alphanumeric() || c == '_') {
            return false;
        }
    }
    true
}
