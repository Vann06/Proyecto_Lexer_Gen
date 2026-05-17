// src/bin/api.rs — Servidor HTTP (Axum) que expone el pipeline LR(1) a la UI
#[path = "../analizador_sintactico/mod.rs"]
mod analizador_sintactico;

use analizador_sintactico::first::calculate_first;
use analizador_sintactico::follow::calculate_follow;
use analizador_sintactico::grammar::{Grammar, Symbol};
use analizador_sintactico::lr1::{LR1Action, LR1Automaton, LR1Tables, TraceStep};

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

// ─────────────────────────────────────────────────────────────────────────────
// Tipos de request
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CompileRequest {
    content: String,
}

#[derive(Deserialize)]
struct ParseRequest {
    content: String,
    tokens:  Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipos de response
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct StateData {
    id:    usize,
    items: Vec<String>,
}

#[derive(Serialize)]
struct ProdData {
    n:   usize,
    lhs: String,
    rhs: Vec<String>,
}

#[derive(Serialize)]
struct ProblemData {
    level: String,
    code:  String,
    msg:   String,
    loc:   String,
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
}

#[derive(Serialize)]
struct ParseResponse {
    trace:    Vec<Value>,
    accepted: bool,
    error:    Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn health() -> impl IntoResponse {
    Json(json!({ "status": "ok", "service": "syntra-api" }))
}

async fn compile_parser(
    Json(req): Json<CompileRequest>,
) -> Result<Json<CompileResponse>, (StatusCode, Json<Value>)> {
    build_compile_response(&req.content)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))))
}

async fn parse_tokens(
    Json(req): Json<ParseRequest>,
) -> Result<Json<ParseResponse>, (StatusCode, Json<Value>)> {
    build_parse_response(&req.content, req.tokens)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))))
}

// ─────────────────────────────────────────────────────────────────────────────
// Lógica de negocio
// ─────────────────────────────────────────────────────────────────────────────

fn build_compile_response(content: &str) -> Result<CompileResponse, String> {
    let grammar   = Grammar::parse_for_lr_from_str(content)?;
    let first_sets  = calculate_first(&grammar);
    let follow_sets = calculate_follow(&grammar, &first_sets);
    let automaton = LR1Automaton::build(&grammar, &first_sets);
    let tables    = LR1Tables::build(&automaton);

    // ── Estados ──────────────────────────────────────────────────────────────
    let mut states: Vec<StateData> = automaton.states.iter().map(|s| {
        let mut items: Vec<_> = s.items.iter().collect();
        items.sort();
        StateData {
            id:    s.id,
            items: items.iter().map(|it| it.display()).collect(),
        }
    }).collect();
    states.sort_by_key(|s| s.id);

    // ── Tabla ACTION ──────────────────────────────────────────────────────────
    let mut action_map: HashMap<String, HashMap<String, String>> = HashMap::new();
    for ((state, terminal), lr_action) in &tables.action {
        let action_str = match lr_action {
            LR1Action::Shift(s) => format!("s{}", s),
            LR1Action::Reduce { head, body } => {
                let b = sym_list_to_str(body);
                format!("r({} → {})", head, b)
            }
            LR1Action::Accept => "acc".to_string(),
        };
        action_map
            .entry(state.to_string())
            .or_default()
            .insert(terminal.clone(), action_str);
    }

    // ── Tabla GOTO ────────────────────────────────────────────────────────────
    let mut goto_map: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for ((state, nt), dest) in &tables.goto {
        goto_map
            .entry(state.to_string())
            .or_default()
            .insert(nt.clone(), *dest);
    }

    // ── Terminales ────────────────────────────────────────────────────────────
    let mut terminals: Vec<String> = grammar.tokens.iter().cloned().collect();
    terminals.push("$".to_string());
    terminals.sort();

    // ── No-terminales (en orden de aparición en la gramática) ─────────────────
    let mut seen = std::collections::HashSet::new();
    let non_terminals: Vec<String> = grammar.productions.iter()
        .filter(|p| seen.insert(p.head.clone()))
        .map(|p| p.head.clone())
        .collect();

    // ── FIRST y FOLLOW (sin ε) ────────────────────────────────────────────────
    let first_map = sets_to_sorted_vecs(&first_sets);
    let follow_map = sets_to_sorted_vecs(&follow_sets);

    // ── Producciones numeradas desde 1 ───────────────────────────────────────
    let mut prods: Vec<ProdData> = Vec::new();
    let mut n = 1usize;
    for prod in &grammar.productions {
        for body in &prod.bodies {
            let rhs = if body.is_empty() {
                vec!["ε".to_string()]
            } else {
                body.iter().map(|s| sym_name(s).to_string()).collect()
            };
            prods.push(ProdData { n, lhs: prod.head.clone(), rhs });
            n += 1;
        }
    }

    // ── Problemas / conflictos ────────────────────────────────────────────────
    let problems = build_problems(&tables.conflicts, automaton.states.len());

    Ok(CompileResponse {
        states,
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

fn build_parse_response(content: &str, tokens: Vec<String>) -> Result<ParseResponse, String> {
    let grammar  = Grammar::parse_for_lr_from_str(content)?;
    let first_sets = calculate_first(&grammar);
    let automaton = LR1Automaton::build(&grammar, &first_sets);
    let tables   = LR1Tables::build(&automaton);

    let steps    = tables.parse_with_trace(tokens);
    let accepted = steps.last().map(|s| s.action == "acc").unwrap_or(false);
    let error    = if !accepted {
        steps.last().map(|s| s.desc.clone())
    } else {
        None
    };

    let trace = steps.iter().map(trace_step_to_json).collect();

    Ok(ParseResponse { trace, accepted, error })
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn sym_name(s: &Symbol) -> &str {
    match s {
        Symbol::Terminal(t) | Symbol::NonTerminal(t) => t.as_str(),
    }
}

fn sym_list_to_str(body: &[Symbol]) -> String {
    if body.is_empty() {
        "ε".to_string()
    } else {
        body.iter().map(sym_name).collect::<Vec<_>>().join(" ")
    }
}

fn sets_to_sorted_vecs(
    sets: &HashMap<String, std::collections::HashSet<String>>,
) -> HashMap<String, Vec<String>> {
    sets.iter()
        .map(|(k, v)| {
            let mut vals: Vec<String> = v.iter()
                .filter(|s| s.as_str() != "ε")
                .cloned()
                .collect();
            vals.sort();
            (k.clone(), vals)
        })
        .collect()
}

fn build_problems(conflicts: &[String], state_count: usize) -> Vec<ProblemData> {
    if conflicts.is_empty() {
        vec![ProblemData {
            level: "info".to_string(),
            code:  "I100".to_string(),
            msg:   format!("gramática es LR(1) sin conflictos · {} estados", state_count),
            loc:   "parser.yalp".to_string(),
        }]
    } else {
        conflicts.iter().enumerate().map(|(i, c)| ProblemData {
            level: "err".to_string(),
            code:  format!("E{:03}", i + 1),
            msg:   c.clone(),
            loc:   "parser.yalp".to_string(),
        }).collect()
    }
}

/// Convierte un TraceStep al formato JSON que espera el frontend:
/// stack = [estado, símbolo, estado, símbolo, …]  (números y strings mezclados)
fn trace_step_to_json(step: &TraceStep) -> Value {
    let mut stack: Vec<Value> = Vec::new();
    for i in 0..step.stack_states.len() {
        stack.push(json!(step.stack_states[i]));
        if i < step.stack_symbols.len() {
            stack.push(json!(step.stack_symbols[i]));
        }
    }
    json!({
        "stack":     stack,
        "remaining": step.remaining,
        "action":    step.action,
        "desc":      step.desc,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health",              get(health))
        .route("/api/parser/compile",  post(compile_parser))
        .route("/api/parser/parse",    post(parse_tokens))
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("Syntra API · http://0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
}
