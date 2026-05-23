// src/bin/api.rs — Servidor HTTP (Axum) que expone el pipeline al frontend
#[path = "../analizador_sintactico/mod.rs"]
mod analizador_sintactico;

use analizador_sintactico::first::calculate_first;
use analizador_sintactico::follow::calculate_follow;
use analizador_sintactico::grammar::{Grammar, Symbol};
use analizador_sintactico::lalr::{LALRItem, merge_by_core};
use analizador_sintactico::ll1::LL1Parser;
use analizador_sintactico::lr0::{LR0Automaton, LR0Item};
use analizador_sintactico::lr1::LR1Automaton;
use analizador_sintactico::tables::{Action, Conflict, LRTable};

use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tower_http::cors::{Any, CorsLayer};

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

fn default_mode() -> String { "lalr".to_string() }

// ─── Response types ───────────────────────────────────────────────────────────

#[derive(Serialize)]
struct StateData { id: usize, items: Vec<String> }

#[derive(Serialize)]
struct ProdData { n: usize, lhs: String, rhs: Vec<String> }

#[derive(Serialize)]
struct ProblemData { level: String, code: String, msg: String, loc: String }

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
}

#[derive(Serialize)]
struct ParseResponse { trace: Vec<Value>, accepted: bool, error: Option<String> }

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

// ─── Business logic ───────────────────────────────────────────────────────────

fn build_compile_response(content: &str, mode: &str) -> Result<CompileResponse, String> {
    if mode == "ll1" {
        return build_compile_ll1(content);
    }

    let grammar     = Grammar::parse_for_lr_from_str(content)?;
    let first_sets  = calculate_first(&grammar);
    let follow_sets = calculate_follow(&grammar, &first_sets);

    let (states_data, table) = if mode == "slr" {
        let lr0   = LR0Automaton::build(&grammar);
        let data  = lr0_states_to_data(&lr0);
        let table = LRTable::build_from_slr(&lr0, &grammar, &follow_sets);
        (data, table)
    } else {
        let lr1   = LR1Automaton::build(&grammar, &first_sets);
        let lalr  = merge_by_core(lr1);
        let data  = lalr_states_to_data(&lalr.states);
        let table = LRTable::build_from_lalr(&lalr, &grammar);
        (data, table)
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
    })
}

fn build_parse_response(content: &str, tokens: Vec<String>, mode: &str) -> Result<ParseResponse, String> {
    if mode == "ll1" {
        let grammar     = Grammar::parse_for_ll1_from_str(content)?;
        let first_sets  = calculate_first(&grammar);
        let follow_sets = calculate_follow(&grammar, &first_sets);
        let parser      = LL1Parser::build(&grammar, &first_sets, &follow_sets)?;

        return match parser.parse(tokens.clone()) {
            Ok(()) => Ok(ParseResponse {
                trace: vec![json!({
                    "stack":     [0],
                    "remaining": ["$"],
                    "action":    "acc",
                    "desc":      "Cadena aceptada (LL(1))",
                })],
                accepted: true,
                error: None,
            }),
            Err(e) => Ok(ParseResponse {
                trace: vec![json!({
                    "stack":     [0],
                    "remaining": tokens,
                    "action":    "error",
                    "desc":      e.clone(),
                })],
                accepted: false,
                error: Some(e),
            }),
        };
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

    Ok(ParseResponse { trace, accepted, error })
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

    let app = Router::new()
        .route("/health",             get(health))
        .route("/api/parser/compile", post(compile_parser))
        .route("/api/parser/parse",   post(parse_tokens))
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Syntra API · http://0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
}
