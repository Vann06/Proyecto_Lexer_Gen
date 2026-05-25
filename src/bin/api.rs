// src/bin/api.rs — Servidor HTTP (Axum) que expone el pipeline al frontend
#[path = "../analizador_sintactico/mod.rs"]
mod analizador_sintactico;
#[path = "../spec/mod.rs"]
mod spec;
#[path = "../regex/mod.rs"]
mod regex;
#[path = "../automata/mod.rs"]
mod automata;
#[path = "../table/mod.rs"]
mod table;
#[path = "../runtime/mod.rs"]
mod runtime;
#[path = "../error.rs"]
mod error;

use analizador_sintactico::first::calculate_first;
use analizador_sintactico::follow::calculate_follow;
use analizador_sintactico::grammar::{Grammar, Symbol};
use analizador_sintactico::lalr::{LALRItem, merge_by_core};
use analizador_sintactico::ll1::LL1Parser;
use analizador_sintactico::lr0::{LR0Automaton, LR0Item};
use analizador_sintactico::lr1::LR1Automaton;
use analizador_sintactico::tables::{Action, Conflict, LRTable};

use axum::{
    extract::{Json, Path},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};

use spec::parser::parse_yalex;
use spec::expand::expand_definitions;
use regex::parser::parse_regex;
use runtime::simulator::{Simulator, LexResult};
use table::transition_table::TransitionTable;

// ─── Request types ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CompileRequest {
    content: String,
    #[serde(default = "default_mode")]
    mode: String,
}

#[derive(Deserialize)]
struct ParseRequest {
    content: String,
    tokens: Vec<String>,
    #[serde(default = "default_mode")]
    mode: String,
}

#[derive(Deserialize)]
struct PipelineRequest {
    yal_content:  String,
    yalp_content: String,
    source:       String,
    #[serde(default = "default_mode")]
    mode: String,
}

fn default_mode() -> String { "lalr".to_string() }

// ─── Response types ───────────────────────────────────────────────────────────

#[derive(Serialize)]
struct StateData { id: usize, items: Vec<String> }

#[derive(Serialize)]
struct ProdData { n: usize, lhs: String, rhs: Vec<String> }

#[derive(Serialize)]
struct ProblemData { level: String, code: String, msg: String, loc: String }

#[derive(Serialize, Default)]
struct ParseResponse {
    trace:     Vec<Value>,
    accepted:  bool,
    error:     Option<String>,
    problems:  Vec<Value>,   // errores léxicos/sintácticos con línea:col
    token_map: Vec<Value>,   // [{kind, lexeme, line, col}] para cada token
}

#[derive(Serialize)]
struct CompileResponse {
    states:        Vec<StateData>,
    action:        HashMap<String, HashMap<String, String>>,
    goto:          HashMap<String, HashMap<String, usize>>,
    terminals:     Vec<String>,
    non_terminals: Vec<String>,
    first:         HashMap<String, Vec<String>>,
    follow:        HashMap<String, Vec<String>>,
    prods:         Vec<ProdData>,
    problems:      Vec<ProblemData>,
    start_symbol:  String,
    lr0_dot:       String,
}

#[derive(Serialize)]
struct WsFile { name: String, kind: String }

// ─── Workspace helpers ────────────────────────────────────────────────────────

fn workspace_dir() -> PathBuf { PathBuf::from("workspace") }

fn sanitize_filename(name: &str) -> Option<String> {
    let allowed = [".yal", ".yalex", ".yalp", ".yapar", ".txt"];
    if name.contains('/') || name.contains('\\') || name.contains("..") { return None; }
    if !allowed.iter().any(|ext| name.ends_with(ext)) { return None; }
    Some(name.to_string())
}

async fn workspace_list() -> impl IntoResponse {
    let dir = workspace_dir();
    let mut files: Vec<WsFile> = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let kind = if name.ends_with(".yal") || name.ends_with(".yalex") { "yal" }
                       else if name.ends_with(".yalp") || name.ends_with(".yapar") { "yalp" }
                       else { "txt" };
            files.push(WsFile { name, kind: kind.to_string() });
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Json(json!({ "files": files }))
}

async fn workspace_read(Path(name): Path<String>) -> impl IntoResponse {
    match sanitize_filename(&name) {
        None => (StatusCode::BAD_REQUEST, "invalid filename".to_string()),
        Some(n) => match fs::read_to_string(workspace_dir().join(&n)) {
            Ok(content) => (StatusCode::OK, content),
            Err(_)      => (StatusCode::NOT_FOUND, "not found".to_string()),
        }
    }
}

async fn workspace_write(Path(name): Path<String>, body: String) -> impl IntoResponse {
    match sanitize_filename(&name) {
        None    => StatusCode::BAD_REQUEST,
        Some(n) => match fs::write(workspace_dir().join(&n), body.as_bytes()) {
            Ok(_)  => StatusCode::OK,
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

async fn health() -> impl IntoResponse {
    Json(json!({ "status": "ok", "service": "syntra-api" }))
}

async fn compile_parser(
    Json(req): Json<CompileRequest>,
) -> Result<Json<CompileResponse>, (StatusCode, Json<Value>)> {
    build_compile_response(&req.content, &req.mode)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))))
}

async fn parse_tokens(
    Json(req): Json<ParseRequest>,
) -> Result<Json<ParseResponse>, (StatusCode, Json<Value>)> {
    build_parse_response(&req.content, req.tokens, &req.mode)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))))
}

async fn run_pipeline(
    Json(req): Json<PipelineRequest>,
) -> Result<Json<ParseResponse>, (StatusCode, Json<Value>)> {
    build_pipeline_response(&req.yal_content, &req.yalp_content, &req.source, &req.mode)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))))
}

// ─── Business logic ───────────────────────────────────────────────────────────

fn build_compile_response(content: &str, mode: &str) -> Result<CompileResponse, String> {
    if mode == "ll1" {
        return build_compile_ll1(content);
    }

    let grammar     = Grammar::parse_for_lr_from_str(content)?;
    let first_sets  = calculate_first(&grammar);
    let follow_sets = calculate_follow(&grammar, &first_sets);

    let (states_data, table, lr0_dot) = if mode == "slr" {
        let lr0   = LR0Automaton::build(&grammar);
        let dot   = lr0_to_dot(&lr0);
        let data  = lr0_states_to_data(&lr0);
        let table = LRTable::build_from_slr(&lr0, &grammar, &follow_sets);
        (data, table, dot)
    } else {
        let lr0   = LR0Automaton::build(&grammar);
        let dot   = lr0_to_dot(&lr0);
        let lr1   = LR1Automaton::build(&grammar, &first_sets);
        let lalr  = merge_by_core(lr1);
        let data  = lalr_states_to_data(&lalr.states);
        let table = LRTable::build_from_lalr(&lalr, &grammar);
        (data, table, dot)
    };

    let action_map = action_table_to_map(&table, &grammar);
    let goto_map   = goto_table_to_map(&table);

    let mut terminals: Vec<String> = grammar.tokens.iter().cloned().collect();
    terminals.push("$".to_string());
    terminals.sort();

    let mut seen = std::collections::HashSet::new();
    let non_terminals: Vec<String> = grammar.productions.iter()
        .filter(|p| seen.insert(p.head.clone()))
        .map(|p| p.head.clone())
        .collect();

    let first_map  = sets_to_sorted_vecs(&first_sets);
    let follow_map = sets_to_sorted_vecs(&follow_sets);
    let prods      = grammar_to_prods(&grammar);
    let state_count = states_data.len();
    let problems   = build_problems(&table.conflicts, state_count, mode);

    Ok(CompileResponse {
        states: states_data,
        action: action_map,
        goto:   goto_map,
        terminals,
        non_terminals,
        first:  first_map,
        follow: follow_map,
        prods,
        problems,
        start_symbol: grammar.start_symbol.clone(),
        lr0_dot,
    })
}

fn build_compile_ll1(content: &str) -> Result<CompileResponse, String> {
    let grammar     = Grammar::parse_for_ll1_from_str(content)?;
    let first_sets  = calculate_first(&grammar);
    let follow_sets = calculate_follow(&grammar, &first_sets);
    let ll1         = LL1Parser::build(&grammar, &first_sets, &follow_sets)?;

    // Representar la tabla M[A,a] como pseudo-estados
    let mut states: Vec<StateData> = grammar.productions.iter().enumerate().map(|(id, prod)| {
        let nt = &prod.head;
        let mut items: Vec<String> = Vec::new();
        if let Some(row) = ll1.table.get(nt) {
            let mut pairs: Vec<(&String, &analizador_sintactico::grammar::Production)> = row.iter().collect();
            pairs.sort_by_key(|(t, _)| t.as_str());
            for (terminal, production) in pairs {
                let rhs: Vec<String> = production.bodies.iter().flat_map(|b| {
                    if b.is_empty() { vec!["ε".to_string()] }
                    else { b.iter().map(|s| sym_name(s).to_string()).collect() }
                }).collect();
                items.push(format!("M[{}, {}] → {}", nt, terminal, rhs.join(" ")));
            }
        }
        StateData { id, items }
    }).collect();
    states.sort_by_key(|s| s.id);

    // ACTION como tabla LL1: id_produccion → terminal → "NT → RHS"
    let mut action_map: HashMap<String, HashMap<String, String>> = HashMap::new();
    for (id, prod) in grammar.productions.iter().enumerate() {
        let nt = &prod.head;
        if let Some(row) = ll1.table.get(nt) {
            for (terminal, production) in row {
                let rhs: Vec<String> = production.bodies.iter().flat_map(|b| {
                    if b.is_empty() { vec!["ε".to_string()] }
                    else { b.iter().map(|s| sym_name(s).to_string()).collect() }
                }).collect();
                action_map.entry(id.to_string()).or_default()
                    .insert(terminal.clone(), format!("{} → {}", nt, rhs.join(" ")));
            }
        }
    }

    let mut terminals: Vec<String> = grammar.tokens.iter().cloned().collect();
    terminals.push("$".to_string());
    terminals.sort();

    let mut seen = std::collections::HashSet::new();
    let non_terminals: Vec<String> = grammar.productions.iter()
        .filter(|p| seen.insert(p.head.clone()))
        .map(|p| p.head.clone())
        .collect();

    let nt_count = non_terminals.len();
    let problems = vec![ProblemData {
        level: "info".to_string(),
        code:  "I100".to_string(),
        msg:   format!("gramática es LL(1) sin conflictos · {} no-terminales", nt_count),
        loc:   "parser.yalp".to_string(),
    }];

    let lr0_dot = Grammar::parse_for_lr_from_str(content)
        .map(|g| { let lr0 = LR0Automaton::build(&g); lr0_to_dot(&lr0) })
        .unwrap_or_default();

    Ok(CompileResponse {
        states,
        action: action_map,
        goto:   HashMap::new(),
        terminals,
        non_terminals,
        first:  sets_to_sorted_vecs(&first_sets),
        follow: sets_to_sorted_vecs(&follow_sets),
        prods:  grammar_to_prods(&grammar),
        problems,
        start_symbol: grammar.start_symbol.clone(),
        lr0_dot,
    })
}

fn build_parse_response(content: &str, tokens: Vec<String>, mode: &str) -> Result<ParseResponse, String> {
    if mode == "ll1" {
        let grammar     = Grammar::parse_for_ll1_from_str(content)?;
        let first_sets  = calculate_first(&grammar);
        let follow_sets = calculate_follow(&grammar, &first_sets);
        let parser      = LL1Parser::build(&grammar, &first_sets, &follow_sets)?;

        let steps    = parser.parse_with_trace(tokens);
        let accepted = steps.last().map(|s| s.action == "acc").unwrap_or(false);
        let error    = if !accepted { steps.last().map(|s| s.desc.clone()) } else { None };
        let trace: Vec<Value> = steps.into_iter().map(|s| json!({
            "stack":     s.stack,
            "remaining": s.remaining,
            "action":    s.action,
            "desc":      s.desc,
        })).collect();
        return Ok(ParseResponse { trace, accepted, error, ..Default::default() });
    }

    let grammar     = Grammar::parse_for_lr_from_str(content)?;
    let first_sets  = calculate_first(&grammar);
    let follow_sets = calculate_follow(&grammar, &first_sets);

    let table = if mode == "slr" {
        let lr0 = LR0Automaton::build(&grammar);
        LRTable::build_from_slr(&lr0, &grammar, &follow_sets)
    } else {
        let lr1  = LR1Automaton::build(&grammar, &first_sets);
        let lalr = merge_by_core(lr1);
        LRTable::build_from_lalr(&lalr, &grammar)
    };

    let trace    = parse_with_trace_lr(&table, tokens);
    let accepted = trace.last()
        .and_then(|s| s["action"].as_str())
        .map(|a| a == "acc")
        .unwrap_or(false);
    let error = if !accepted {
        trace.last().and_then(|s| s["desc"].as_str()).map(String::from)
    } else {
        None
    };

    Ok(ParseResponse { trace, accepted, error, ..Default::default() })
}

// ─── Pipeline: lex → parse ───────────────────────────────────────────────────

fn build_pipeline_response(yal: &str, yalp: &str, source: &str, mode: &str) -> Result<ParseResponse, String> {
    let lexer_table = build_lexer_table_from_str(yal)?;

    // Lex the source
    let grammar_for_filter = Grammar::parse_for_lr_from_str(yalp)?;
    let mut sim = Simulator::new(&lexer_table, source);
    let mut token_kinds: Vec<String> = Vec::new();
    let mut token_map: Vec<(String, String, usize, usize)> = Vec::new(); // (kind, lexeme, line, col)
    let mut lex_problems: Vec<Value> = Vec::new();

    loop {
        match sim.next_token() {
            LexResult::Token(t) => {
                let k = lex_normalize_kind(&t.kind);
                if !lex_is_ignored(&k, &grammar_for_filter) {
                    token_map.push((k.clone(), t.lexeme.clone(), t.line, t.col));
                    token_kinds.push(k);
                }
            }
            LexResult::Error { lexeme, line, col } => {
                lex_problems.push(json!({
                    "level": "err",
                    "code":  "L001",
                    "msg":   format!("Carácter no reconocido: '{}'", lexeme),
                    "loc":   format!("línea {} · col {}", line, col),
                    "line":  line,
                    "col":   col
                }));
            }
            LexResult::EOF => break,
        }
    }

    let mut response = build_parse_response(yalp, token_kinds, mode)?;

    // Localizar el token que causó el error sintáctico
    if !response.accepted {
        let consumed = response.trace.iter()
            .filter(|s| {
                let a = s.get("action").and_then(|v| v.as_str()).unwrap_or("");
                a == "match" || a.starts_with('s')
            })
            .count();
        let loc_entry = token_map.get(consumed)
            .or_else(|| token_map.last());
        if let Some((kind, lexeme, line, col)) = loc_entry {
            lex_problems.push(json!({
                "level": "err",
                "code":  "P001",
                "msg":   format!("Error sintáctico · token '{}' (\"{}\")", kind, lexeme),
                "loc":   format!("línea {} · col {}", line, col),
                "line":  line,
                "col":   col
            }));
        }
    }

    let token_map_json: Vec<Value> = token_map.iter()
        .map(|(k, lx, l, c)| json!({"kind": k, "lexeme": lx, "line": l, "col": c}))
        .collect();

    response.problems  = lex_problems;
    response.token_map = token_map_json;
    Ok(response)
}

fn build_lexer_table_from_str(yal_src: &str) -> Result<TransitionTable, String> {
    let spec     = parse_yalex(yal_src).map_err(|e| format!("Error al parsear .yal: {}", e))?;
    let expanded = expand_definitions(&spec);

    let mut id_counter = 0usize;
    let mut nfas = Vec::new();
    for rule in &expanded {
        let ast = parse_regex(&rule.pattern_expanded)
            .map_err(|e| format!("Error en regex '{}': {}", rule.pattern_expanded, e))?;
        let mut nfa = automata::nfa::build_nfa_from_ast(&ast, &mut id_counter);
        if let Some(fs) = nfa.states.get_mut(&nfa.end_state) {
            fs.accept_action = Some((rule.priority, rule.action_code.clone()));
        }
        nfas.push(nfa);
    }
    let master  = automata::nfa::combine_nfas(nfas, &mut id_counter);
    let dfa     = automata::subset::build_dfa_from_nfa(&master);
    let min_dfa = automata::minimize::minimize_dfa(&dfa);
    Ok(table::transition_table::build(&min_dfa))
}

fn lex_normalize_kind(kind: &str) -> String { kind.to_uppercase() }

fn lex_is_ignored(kind: &str, grammar: &Grammar) -> bool {
    let lower = kind.to_lowercase();
    lower.contains("whitespace") || lower.contains("comment") || lower == "ignored"
        || grammar.ignores.contains(kind)
}

// ─── LR parse trace ───────────────────────────────────────────────────────────

fn parse_with_trace_lr(table: &LRTable, tokens: Vec<String>) -> Vec<Value> {
    let mut state_stack:  Vec<usize>  = vec![table.start_state];
    let mut symbol_stack: Vec<String> = Vec::new();
    let mut input: Vec<String> = tokens;
    input.push("$".to_string());
    let mut ip   = 0usize;
    let mut done = false;
    let mut trace: Vec<Value> = Vec::new();

    while !done {
        let s = *state_stack.last().unwrap();
        let a = input[ip].clone();

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
                let body_str = sym_list_to_str(&body);
                for _ in 0..body.len() {
                    state_stack.pop();
                    symbol_stack.pop();
                }
                let top = *state_stack.last().unwrap();
                if let Some(&next) = table.goto.get(&(top, head.clone())) {
                    state_stack.push(next);
                    symbol_stack.push(head.clone());
                }
                ("r".to_string(), format!("{} → {}", head, body_str))
            }
            Some(Action::Accept) => {
                done = true;
                ("acc".to_string(), "Cadena aceptada".to_string())
            }
            None => {
                done = true;
                ("error".to_string(), format!("Error sintáctico en I{}, token '{}'", s, a))
            }
        };

        trace.push(json!({
            "stack":     stack_val,
            "remaining": remaining,
            "action":    action_str,
            "desc":      desc,
        }));
    }
    trace
}

// ─── LR(0) DOT generation ────────────────────────────────────────────────────

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
        let (af, _) = a.0; let (bf, _) = b.0;
        af.cmp(bf).then(a.1.cmp(b.1))
    });

    for ((from, sym), to) in transitions {
        let sym_label = sym_name(sym).replace('"', "\\\"");
        dot.push_str(&format!("  {} -> {} [label=\"{}\"];\n", from, to, sym_label));
    }

    dot.push_str("}\n");
    dot
}

// ─── State formatting ─────────────────────────────────────────────────────────

fn lalr_states_to_data(states: &[analizador_sintactico::lalr::LALRState]) -> Vec<StateData> {
    let mut v: Vec<StateData> = states.iter().map(|s| {
        let mut items: Vec<String> = s.items.iter().map(format_lalr_item).collect();
        items.sort();
        StateData { id: s.id, items }
    }).collect();
    v.sort_by_key(|s| s.id);
    v
}

fn lr0_states_to_data(automaton: &LR0Automaton) -> Vec<StateData> {
    let mut v: Vec<StateData> = automaton.states.iter().map(|s| {
        let mut items: Vec<String> = s.items.iter().map(format_lr0_item).collect();
        items.sort();
        StateData { id: s.id, items }
    }).collect();
    v.sort_by_key(|s| s.id);
    v
}

fn format_lalr_item(it: &LALRItem) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, sym) in it.body.iter().enumerate() {
        if i == it.dot_pos { parts.push("•".to_string()); }
        parts.push(sym_name(sym).to_string());
    }
    if it.dot_pos == it.body.len() { parts.push("•".to_string()); }
    let mut las: Vec<&str> = it.lookaheads.iter().map(|s| s.as_str()).collect();
    las.sort();
    format!("[{} -> {}, {{{}}}]", it.head, parts.join(" "), las.join(","))
}

fn format_lr0_item(it: &LR0Item) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, sym) in it.body.iter().enumerate() {
        if i == it.dot_pos { parts.push("•".to_string()); }
        parts.push(sym_name(sym).to_string());
    }
    if it.dot_pos == it.body.len() { parts.push("•".to_string()); }
    format!("[{} -> {}]", it.head, parts.join(" "))
}

// ─── Table helpers ────────────────────────────────────────────────────────────

fn action_table_to_map(table: &LRTable, _grammar: &Grammar) -> HashMap<String, HashMap<String, String>> {
    let mut map: HashMap<String, HashMap<String, String>> = HashMap::new();
    for ((state, terminal), action) in &table.action {
        let s = match action {
            Action::Shift(n)              => format!("s{}", n),
            Action::Reduce { head, body } => format!("r({} → {})", head, sym_list_to_str(body)),
            Action::Accept                => "acc".to_string(),
        };
        map.entry(state.to_string()).or_default().insert(terminal.clone(), s);
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

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn sym_name(s: &Symbol) -> &str {
    match s { Symbol::Terminal(t) | Symbol::NonTerminal(t) => t.as_str() }
}

fn sym_list_to_str(body: &[Symbol]) -> String {
    if body.is_empty() { "ε".to_string() }
    else { body.iter().map(sym_name).collect::<Vec<_>>().join(" ") }
}

fn sets_to_sorted_vecs(
    sets: &HashMap<String, std::collections::HashSet<String>>,
) -> HashMap<String, Vec<String>> {
    sets.iter()
        .map(|(k, v)| {
            let mut vals: Vec<String> = v.iter().filter(|s| s.as_str() != "ε").cloned().collect();
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
            let rhs = if body.is_empty() { vec!["ε".to_string()] }
                      else { body.iter().map(|s| sym_name(s).to_string()).collect() };
            prods.push(ProdData { n, lhs: prod.head.clone(), rhs });
            n += 1;
        }
    }
    prods
}

fn build_problems(conflicts: &[Conflict], state_count: usize, mode: &str) -> Vec<ProblemData> {
    if conflicts.is_empty() {
        vec![ProblemData {
            level: "info".to_string(),
            code:  "I100".to_string(),
            msg:   format!("gramática {} sin conflictos · {} estados", mode.to_uppercase(), state_count),
            loc:   "parser.yalp".to_string(),
        }]
    } else {
        conflicts.iter().enumerate().map(|(i, c)| ProblemData {
            level: "warn".to_string(),
            code:  format!("W{:03}", i + 1),
            msg:   c.describe(),
            loc:   "parser.yalp".to_string(),
        }).collect()
    }
}

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Inicializar workspace con archivos por defecto
    let ws = workspace_dir();
    fs::create_dir_all(&ws).ok();
    let defaults: &[(&str, &str)] = &[
        ("lexer.yal",   "let digit = ['0'-'9']\nlet letter = ['a'-'z' 'A'-'Z']\nlet id = letter (letter|digit)*\nrule tokens =\n  | id         { return ID }\n  | digit+     { return NUM }\n  | '+'        { return PLUS }\n  | '*'        { return STAR }\n  | ' '        { skip }"),
        ("parser.yalp", "%token c d\n%%\nS : C C ;\nC : c C\n  | d\n;"),
        ("input.txt",   "c c d c d\nc d d\nd c d"),
    ];
    for (name, content) in defaults {
        let path = ws.join(name);
        if !path.exists() { fs::write(&path, content).ok(); }
    }

    let app = Router::new()
        .route("/health",              get(health))
        .route("/api/parser/compile",  post(compile_parser))
        .route("/api/parser/parse",    post(parse_tokens))
        .route("/api/pipeline",        post(run_pipeline))
        .route("/api/workspace",       get(workspace_list))
        .route("/api/workspace/:name", get(workspace_read).put(workspace_write))
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Syntra API · http://0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
}
