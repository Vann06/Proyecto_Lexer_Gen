// src/bin/api.rs — Servidor HTTP (Axum) que expone el pipeline al frontend
use lexer_generator::api;

use axum::{
    extract::{Json, Path},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
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

#[derive(Deserialize)]
struct PipelineRequest {
    yal_content:  String,
    yalp_content: String,
    source:       String,
    #[serde(default = "default_mode")]
    mode: String,
}

fn default_mode() -> String { "lalr".to_string() }

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
) -> Result<Json<api::CompileResponse>, (StatusCode, Json<Value>)> {
    api::build_compile_response(&req.content, &req.mode)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))))
}

async fn parse_tokens(
    Json(req): Json<ParseRequest>,
) -> Result<Json<api::ParseResponse>, (StatusCode, Json<Value>)> {
    api::build_parse_response(&req.content, req.tokens, &req.mode)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))))
}

async fn run_pipeline(
    Json(req): Json<PipelineRequest>,
) -> Result<Json<api::ParseResponse>, (StatusCode, Json<Value>)> {
    api::build_pipeline_response(&req.yal_content, &req.yalp_content, &req.source, &req.mode)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))))
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
        ("parser.yalp", "%token ID NUM PLUS STAR\n%start E\n%%\nE : E PLUS T\n  | T\n  ;\nT : T STAR F\n  | F\n  ;\nF : ID\n  | NUM\n  ;\n"),
        ("input.txt",   "a + b\n3 * x\na + b * c"),
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
