// API logic shared by the HTTP server and tests.

use crate::analizador_sintactico::first::calculate_first;
use crate::analizador_sintactico::follow::calculate_follow;
use crate::analizador_sintactico::grammar::{body_to_string, Grammar};
use crate::analizador_sintactico::lalr::{merge_by_core, LALRItem};
use crate::analizador_sintactico::ll1::LL1Parser;
use crate::analizador_sintactico::lr0::{LR0Automaton, LR0Item};
use crate::analizador_sintactico::lr1::LR1Automaton;
use crate::analizador_sintactico::parse_tree::ParseToken;
use crate::analizador_sintactico::parser_lr::LRParser;
use crate::analizador_sintactico::tables::{format_expected_tokens, Action, Conflict, LRTable};
use crate::regex::parser::parse_regex;
use crate::runtime::simulator::{LexResult, Simulator};
use crate::spec::expand::expand_definitions;
use crate::spec::parser::parse_yalex;
use crate::table::transition_table::TransitionTable;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

#[derive(Serialize)]
pub struct StateData {
    pub id: usize,
    pub items: Vec<String>,
}

#[derive(Serialize)]
pub struct ProdData {
    pub n: usize,
    pub lhs: String,
    pub rhs: Vec<String>,
}

#[derive(Serialize)]
pub struct ProblemData {
    pub level: String,
    pub code: String,
    pub msg: String,
    pub loc: String,
}

#[derive(Serialize, Default)]
pub struct ParseResponse {
    pub trace: Vec<Value>,
    pub accepted: bool,
    pub error: Option<String>,
    pub problems: Vec<Value>,
    pub token_map: Vec<Value>,
}

#[derive(Serialize)]
pub struct CompileResponse {
    pub states: Vec<StateData>,
    pub action: HashMap<String, HashMap<String, String>>,
    pub goto: HashMap<String, HashMap<String, usize>>,
    pub terminals: Vec<String>,
    pub non_terminals: Vec<String>,
    pub first: HashMap<String, Vec<String>>,
    pub follow: HashMap<String, Vec<String>>,
    pub prods: Vec<ProdData>,
    pub problems: Vec<ProblemData>,
    pub start_symbol: String,
    pub lr0_dot: String,
}

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
    let first_sets = calculate_first(&grammar);
    let follow_sets = calculate_follow(&grammar, &first_sets);

    let table = if mode == "slr" {
        let lr0 = LR0Automaton::build(&grammar);
        LRTable::build_from_slr(&lr0, &grammar, &follow_sets)
    } else {
        let lr1 = LR1Automaton::build(&grammar, &first_sets);
        let lalr = merge_by_core(lr1);
        LRTable::build_from_lalr(&lalr, &grammar)
    };

    let trace = parse_with_trace_lr(&table, tokens);
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

    Ok(ParseResponse {
        trace,
        accepted,
        error,
        ..Default::default()
    })
}

pub fn build_pipeline_response(
    yal: &str,
    yalp: &str,
    source: &str,
    mode: &str,
) -> Result<ParseResponse, String> {
    let lexer_table = build_lexer_table_from_str(yal)?;

    // Lex the source (normalize CRLF to LF to avoid treating '\r' as invalid char)
    let source_clean = source.replace('\r', "");
    let grammar_for_filter = Grammar::parse_for_lr_from_str(yalp)?;
    let mut sim = Simulator::new(&lexer_table, &source_clean);
    let mut token_map: Vec<(String, String, usize, usize)> = Vec::new();
    let mut lex_problems: Vec<Value> = Vec::new();

    // Tokenize whole source but keep token positions (line/col).
    loop {
        match sim.next_token() {
            LexResult::Token(t) => {
                let k = lex_normalize_kind(&t.kind);
                if !lex_is_ignored(&k, &grammar_for_filter) {
                    token_map.push((k.clone(), t.lexeme.clone(), t.line, t.col));
                }
            }
            LexResult::Error { lexeme, line, col } => {
                lex_problems.push(json!({
                    "level": "err",
                    "code": "L001",
                    "msg": format!("Carácter no reconocido: '{}'", lexeme),
                    "loc": format!("input.txt:{}:{}", line, col),
                    "line": line,
                    "col": col
                }));
            }
            LexResult::EOF => break,
        }
    }

    // Parse the whole token stream as ONE input — not grouped by physical line.
    // A source can be a single multi-line program (a line break is not a statement
    // boundary in general); callers that want several independent test cases already
    // send one call per case (frontend/IDE/app.jsx's handleParse sends one line at a
    // time, and tests/run_examples_cases.rs iterates `src.lines()` itself). Splitting
    // internally here both mis-locates errors in genuine multi-line input and rebuilds
    // the whole parse table once per line for no reason.
    let token_kinds: Vec<String> = token_map.iter().map(|(k, _, _, _)| k.clone()).collect();

    let mut response = ParseResponse::default();
    response.accepted = true;

    if !token_kinds.is_empty() {
        let resp = build_parse_response(yalp, token_kinds.clone(), mode)?;
        response.accepted = resp.accepted;

        // Attach the source line of the token each step is about to consume, for the
        // UI to highlight while stepping through the trace.
        response.trace = resp
            .trace
            .iter()
            .map(|t| {
                let consumed = t
                    .get("remaining")
                    .and_then(|r| r.as_array())
                    .map(|arr| token_kinds.len().saturating_sub(arr.len()))
                    .unwrap_or(0);
                let line = token_map
                    .get(consumed)
                    .or_else(|| token_map.last())
                    .map(|tk| tk.2)
                    .unwrap_or(1);
                let mut entry = json!({ "line": line });
                if let Some(stack) = t.get("stack") { entry["stack"] = stack.clone(); }
                if let Some(remaining) = t.get("remaining") { entry["remaining"] = remaining.clone(); }
                if let Some(action) = t.get("action") { entry["action"] = action.clone(); }
                if let Some(desc) = t.get("desc") { entry["desc"] = desc.clone(); }
                entry
            })
            .collect();

        if !resp.accepted {
            response.error = Some("Error sintáctico".to_string());

            // Panic-mode recovery (only available for LALR/SLR — LL(1) has no
            // LRTable/LRParser) reports EVERY syntax error in one pass instead of
            // just the first (B2: parse_recovering_with_pos existed but nothing
            // called it). Fall back to the single first-error report — using the
            // same "which token was the driver stuck on" heuristic as before — when
            // recovery isn't available or finds nothing (e.g. LL(1) mode).
            let recovered = collect_lr_syntax_errors(yalp, &token_map, mode);
            if !recovered.is_empty() {
                for (kind, lexeme, line, col, msg) in &recovered {
                    push_syntax_problem(&mut lex_problems, kind, lexeme, *line, *col, msg, &grammar_for_filter.tokens);
                }
            } else {
                // Compute which token was consumed according to the trace's remaining length
                let consumed = resp
                    .trace
                    .last()
                    .and_then(|last| {
                        last.get("remaining").and_then(|r| r.as_array().map(|arr| token_kinds.len().saturating_sub(arr.len())))
                    })
                    .unwrap_or_else(|| {
                        resp.trace
                            .iter()
                            .filter(|s| {
                                let a = s.get("action").and_then(|v| v.as_str()).unwrap_or("");
                                a == "match" || a.starts_with('s')
                            })
                            .count()
                    });

                let loc_entry = token_map.get(consumed).or_else(|| token_map.last());
                if let Some((kind, lexeme, line, col)) = loc_entry {
                    let base_msg = resp.error.clone().unwrap_or_else(|| "Error sintáctico".to_string());
                    push_syntax_problem(&mut lex_problems, kind, lexeme, *line, *col, &base_msg, &grammar_for_filter.tokens);
                }
            }
        }
    }

    let token_map_json: Vec<Value> = token_map
        .iter()
        .map(|(k, lx, l, c)| json!({"kind": k, "lexeme": lx, "line": l, "col": c}))
        .collect();

    response.problems = lex_problems;
    response.token_map = token_map_json;
    Ok(response)
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
                let mut pairs: Vec<(&String, &crate::analizador_sintactico::grammar::Production)> =
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
fn collect_lr_syntax_errors(
    yalp: &str,
    token_map: &[(String, String, usize, usize)],
    mode: &str,
) -> Vec<(String, String, usize, usize, String)> {
    if mode != "lalr" && mode != "slr" {
        return Vec::new();
    }
    let Ok(grammar) = Grammar::parse_for_lr_from_str(yalp) else { return Vec::new(); };
    let first_sets = calculate_first(&grammar);
    let table = if mode == "slr" {
        let follow_sets = calculate_follow(&grammar, &first_sets);
        let lr0 = LR0Automaton::build(&grammar);
        LRTable::build_from_slr(&lr0, &grammar, &follow_sets)
    } else {
        let lr1 = LR1Automaton::build(&grammar, &first_sets);
        let lalr = merge_by_core(lr1);
        LRTable::build_from_lalr(&lalr, &grammar)
    };

    let parse_tokens: Vec<ParseToken> = token_map
        .iter()
        .map(|(k, lx, _, _)| ParseToken { kind: k.clone(), lexeme: lx.clone() })
        .collect();
    // Cualquier token declarado sirve como punto de sincronización: en cuanto se
    // descarta la entrada hasta encontrar UNO que el estado recuperado pueda
    // aceptar, se reintenta desde ahí — una heurística genérica razonable sin
    // conocer de antemano la estructura de la gramática (p. ej. cuál token hace
    // de separador de sentencias).
    let sync: Vec<&str> = grammar.tokens.iter().map(|t| t.as_str()).collect();

    let parser = LRParser::new(&table);
    let (_, errors) = parser.parse_recovering_with_pos(parse_tokens, &sync);

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

/// Enriquece un error de sintaxis con una sugerencia de token por distancia de
/// Levenshtein (si el lexema se parece a algún token declarado) y lo empuja como
/// entrada de `problems`. Compartido entre el reporte de un solo error y el modo
/// pánico multi-error (B2) para que ambos caminos generen el mismo formato.
fn push_syntax_problem(
    lex_problems: &mut Vec<Value>,
    kind: &str,
    lexeme: &str,
    line: usize,
    col: usize,
    base_msg: &str,
    tokens: &HashSet<String>,
) {
    let suggestion = suggest_similar_token(lexeme, tokens);
    if let Some(s) = suggestion {
        if is_identifier_like(lexeme) && s != kind {
            lex_problems.push(json!({
                "level": "err",
                "code": "L001",
                "msg": format!("Lexema posiblemente mal escrito: '{}' (¿quiso '{}'?)", lexeme, s),
                "loc": format!("input.txt:{}:{}", line, col),
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
                "loc": format!("input.txt:{}:{}", line, col),
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
            "loc": format!("input.txt:{}:{}", line, col),
            "line": line,
            "col": col,
            "token": kind,
        }));
    }
}

fn build_lexer_table_from_str(yal_src: &str) -> Result<TransitionTable, String> {
    let spec = parse_yalex(yal_src).map_err(|e| format!("Error al parsear .yal: {}", e))?;
    let expanded = expand_definitions(&spec);

    let mut id_counter = 0usize;
    let mut nfas = Vec::new();
    for rule in &expanded {
        let ast = parse_regex(&rule.pattern_expanded)
            .map_err(|e| format!("Error en regex '{}': {}", rule.pattern_expanded, e))?;
        let mut nfa = crate::automata::nfa::build_nfa_from_ast(&ast, &mut id_counter);
        if let Some(fs) = nfa.states.get_mut(&nfa.end_state) {
            fs.accept_action = Some((rule.priority, rule.action_code.clone()));
        }
        nfas.push(nfa);
    }
    let master = crate::automata::nfa::combine_nfas(nfas, &mut id_counter);
    let dfa = crate::automata::subset::build_dfa_from_nfa(&master);
    let min_dfa = crate::automata::minimize::minimize_dfa(&dfa);
    Ok(crate::table::transition_table::build(&min_dfa))
}

fn lex_normalize_kind(kind: &str) -> String {
    kind.to_uppercase()
}

fn lex_is_ignored(kind: &str, grammar: &Grammar) -> bool {
    let lower = kind.to_lowercase();
    lower.contains("whitespace") || lower.contains("comment") || lower == "ignored" || grammar.ignores.contains(kind)
}

fn parse_with_trace_lr(table: &LRTable, tokens: Vec<String>) -> Vec<Value> {
    let mut state_stack: Vec<usize> = vec![table.start_state];
    let mut symbol_stack: Vec<String> = Vec::new();
    let mut input: Vec<String> = tokens;
    input.push("$".to_string());
    let mut ip = 0usize;
    let mut done = false;
    let mut trace: Vec<Value> = Vec::new();

    while !done {
        let s = *state_stack.last().unwrap();
        // `$` is rejected as a token name at grammar-parse time (Grammar::validate),
        // so no Shift can ever advance `ip` past it — but index defensively instead
        // of panicking the request thread if that invariant is ever broken (A5).
        let a = match input.get(ip) {
            Some(t) => t.clone(),
            None => {
                trace.push(json!({
                    "stack": Value::Null,
                    "remaining": Value::Array(vec![]),
                    "action": "error",
                    "desc": "Error interno: se agotó la entrada de forma inesperada.",
                }));
                break;
            }
        };

        // Snapshot BEFORE the action
        let remaining: Vec<Value> = input[ip..].iter().map(|t| json!(t)).collect();
        let mut stack_val: Vec<Value> = Vec::new();
        for i in 0..state_stack.len() {
            stack_val.push(json!(state_stack[i]));
            if i < symbol_stack.len() {
                stack_val.push(json!(symbol_stack[i]));
            }
        }

        // Clone to release the borrow on table.action before accessing table.goto
        let action_cloned = table.action.get(&(s, a.clone())).cloned();

        let (action_str, desc) = match action_cloned {
            Some(Action::Shift(t)) => {
                state_stack.push(t);
                symbol_stack.push(a.clone());
                ip += 1;
                (format!("s{}", t), format!("Shift '{}' → I{}", a, t))
            }
            Some(Action::Reduce { head, body }) => {
                let body_str = body_to_string(&body);
                for _ in 0..body.len() {
                    state_stack.pop();
                    symbol_stack.pop();
                }
                let top = *state_stack.last().unwrap();
                match table.goto.get(&(top, head.clone())) {
                    Some(&next) => {
                        state_stack.push(next);
                        symbol_stack.push(head.clone());
                        ("r".to_string(), format!("{} → {}", head, body_str))
                    }
                    None => {
                        // Tabla interna inconsistente: sin esto, el bucle seguía con la
                        // pila ya reducida pero sin avanzar, creciendo `trace` sin fin (A12).
                        done = true;
                        (
                            "error".to_string(),
                            format!("Error interno: GOTO[I{}, {}] no definido tras reducción.", top, head),
                        )
                    }
                }
            }
            Some(Action::Accept) => {
                done = true;
                ("acc".to_string(), "Cadena aceptada".to_string())
            }
            None => {
                done = true;
                let expected_str = format_expected_tokens(&table.expected_tokens(s));
                (
                    "error".to_string(),
                    format!(
                        "Error sintáctico en I{}, token '{}'. Esperado: {}",
                        s, a, expected_str
                    ),
                )
            }
        };

        trace.push(json!({
            "stack": stack_val,
            "remaining": remaining,
            "action": action_str,
            "desc": desc,
        }));
    }
    trace
}

fn lr0_to_dot(automaton: &LR0Automaton) -> String {
    let mut dot = String::new();
    dot.push_str("digraph LR0 {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  bgcolor=\"#0d0613\";\n");
    dot.push_str("  node [shape=box fontname=\"Courier\" fontsize=9 color=\"#c026d3\" fontcolor=\"#e8d6f0\" style=filled fillcolor=\"#100817\"];\n");
    dot.push_str("  edge [fontname=\"Courier\" fontsize=9 color=\"#c026d3\" fontcolor=\"#f9a8d4\" arrowsize=0.7];\n");

    let mut sorted_states: Vec<_> = automaton.states.iter().collect();
    sorted_states.sort_by_key(|s| s.id);

    for state in sorted_states {
        let mut items: Vec<String> = state.items.iter().map(|it| format_lr0_item(it)).collect();
        items.sort();
        let label_body = items.join("\\l");
        let label = format!("I{}\\l{}\\l", state.id, label_body).replace('"', "\\\"");

        if state.id == 0 {
            dot.push_str(&format!(
                "  {} [label=\"{}\" color=\"#22d3ee\" fillcolor=\"#0a2530\"];\n",
                state.id, label
            ));
        } else {
            dot.push_str(&format!("  {} [label=\"{}\"];\n", state.id, label));
        }
    }

    let mut transitions: Vec<_> = automaton.transitions.iter().collect();
    transitions.sort_by(|a, b| {
        let (af, _) = a.0;
        let (bf, _) = b.0;
        af.cmp(bf).then(a.1.cmp(b.1))
    });

    for ((from, sym), to) in transitions {
        let sym_label = sym.to_string().replace('"', "\\\"");
        dot.push_str(&format!("  {} -> {} [label=\"{}\"];\n", from, to, sym_label));
    }

    dot.push_str("}\n");
    dot
}

fn lalr_states_to_data(states: &[crate::analizador_sintactico::lalr::LALRState]) -> Vec<StateData> {
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

fn format_lr0_item(it: &LR0Item) -> String {
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
