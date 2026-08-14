// Driver shift-reduce GENÉRICO para cualquier parser LR.
use crate::sintactico::gramatica::grammar::Symbol;
use crate::sintactico::runtime::parse_tree::{ParseNode, ParseToken};
use crate::sintactico::tablas::{Action, LRTable};

#[derive(Debug, Clone)]
pub enum ParseStep {
    Shift { state: usize, token: String },
    Reduce { head: String, body: Vec<Symbol> },
    Accept,
}

#[derive(Debug, Clone)]
pub struct ParseErrorDetail {
    pub pos: usize,
    pub token: String,
    pub msg: String,
}

pub struct LRParser<'a> {
    pub table: &'a LRTable,
}

impl<'a> LRParser<'a> {
    pub fn new(table: &'a LRTable) -> Self {
        LRParser { table }
    }

    /// Ejecuta shift-reduce. `tokens` debe ser una lista de terminales SIN el $ final.
    /// Devuelve la traza de pasos o un mensaje de error sintáctico.
    pub fn parse(&self, tokens: Vec<String>) -> Result<Vec<ParseStep>, String> {
        let mut state_stack: Vec<usize> = vec![self.table.start_state];
        let mut symbol_stack: Vec<Symbol> = Vec::new();

        let mut input: Vec<String> = tokens;
        input.push("$".to_string());
        let mut ip = 0usize;

        let mut trace: Vec<ParseStep> = Vec::new();

        loop {
            let s = *state_stack.last().unwrap();
            // '$' is rejected as a token name at grammar-parse time, so no Shift can
            // ever push `ip` past it — index defensively anyway instead of panicking (A5).
            let a = input.get(ip).ok_or_else(|| {
                "Error interno: se agotó la entrada de forma inesperada.".to_string()
            })?;

            match self.table.action.get(&(s, a.clone())) {
                Some(Action::Shift(t)) => {
                    let t = *t;
                    trace.push(ParseStep::Shift { state: t, token: a.clone() });
                    state_stack.push(t);
                    symbol_stack.push(Symbol::Terminal(a.clone()));
                    ip += 1;
                }
                Some(Action::Reduce { head, body }) => {
                    let head = head.clone();
                    let body = body.clone();

                    trace.push(ParseStep::Reduce { head: head.clone(), body: body.clone() });

                    for _ in 0..body.len() {
                        state_stack.pop();
                        symbol_stack.pop();
                    }

                    let top = *state_stack.last().unwrap();
                    let next_state = self.table.goto.get(&(top, head.clone())).copied()
                        .ok_or_else(|| format!(
                            "Error interno: GOTO[I{}, {}] no definido tras reducción.", top, head
                        ))?;

                    state_stack.push(next_state);
                    symbol_stack.push(Symbol::NonTerminal(head));
                }
                Some(Action::Accept) => {
                    trace.push(ParseStep::Accept);
                    return Ok(trace);
                }
                None => {
                    return Err(format_error(s, a, &self.table));
                }
            }
        }
    }

    /// Ejecuta shift-reduce y construye el árbol de derivación BOTTOM-UP.
    /// Mantiene una pila paralela de `ParseNode`:
    ///   - Shift token t        → push hoja(t)
    ///   - Reduce A → α (|α|=k) → pop k nodos, push interno(A, esos k nodos)
    ///   - Accept               → la pila contiene la raíz
    pub fn parse_tree(&self, tokens: Vec<ParseToken>) -> Result<ParseNode, String> {
        let mut state_stack: Vec<usize> = vec![self.table.start_state];
        let mut node_stack: Vec<ParseNode> = Vec::new();

        let mut input = tokens;
        // Centinela $ con lexema vacío (no aparece en el árbol porque Accept no lo consume).
        input.push(ParseToken { kind: "$".to_string(), lexeme: String::new() });
        let mut ip = 0usize;

        loop {
            let s = *state_stack.last().unwrap();
            // '$' is rejected as a token name at grammar-parse time, so no Shift can
            // ever push `ip` past it — index defensively anyway instead of panicking (A5).
            let current = input.get(ip).ok_or_else(|| {
                "Error interno: se agotó la entrada de forma inesperada.".to_string()
            })?;
            let a = &current.kind;

            match self.table.action.get(&(s, a.clone())) {
                Some(Action::Shift(t)) => {
                    let t = *t;
                    node_stack.push(ParseNode::leaf(current));
                    state_stack.push(t);
                    ip += 1;
                }
                Some(Action::Reduce { head, body }) => {
                    let head = head.clone();
                    let k = body.len();

                    // Pop k nodos hijos en el orden en que están en el cuerpo.
                    let split_at = node_stack.len().saturating_sub(k);
                    let children: Vec<ParseNode> = node_stack.drain(split_at..).collect();

                    for _ in 0..k {
                        state_stack.pop();
                    }

                    let top = *state_stack.last().unwrap();
                    let next_state = self.table.goto.get(&(top, head.clone())).copied()
                        .ok_or_else(|| format!(
                            "Error interno: GOTO[I{}, {}] no definido tras reducción.", top, head
                        ))?;

                    // Si el cuerpo era ε, añadir un nodo ε visible en el árbol.
                    let children = if children.is_empty() {
                        vec![ParseNode::epsilon_leaf()]
                    } else {
                        children
                    };

                    node_stack.push(ParseNode::internal(head, children));
                    state_stack.push(next_state);
                }
                Some(Action::Accept) => {
                    // La pila debe contener exactamente el árbol del símbolo inicial.
                    return node_stack.pop()
                        .ok_or_else(|| "Error interno: Accept con pila de nodos vacía.".to_string());
                }
                None => {
                    return Err(format_error(s, a, &self.table));
                }
            }
        }
    }

  
    ///   1. Registra el error.
    ///   2. Descarta tokens del input hasta encontrar un símbolo de sincronización.
    ///   3. Desapila estados hasta encontrar uno que acepte ese símbolo.
    ///   4. Retoma el parseo desde ahí.
    pub fn parse_recovering(
        &self,
        tokens: Vec<ParseToken>,
        sync: &[&str],
    ) -> (Option<ParseNode>, Vec<String>) {
        let (tree, errors) = self.parse_recovering_with_pos(tokens, sync);
        (tree, errors.into_iter().map(|e| e.msg).collect())
    }

    pub fn parse_recovering_with_pos(
        &self,
        tokens: Vec<ParseToken>,
        sync: &[&str],
    ) -> (Option<ParseNode>, Vec<ParseErrorDetail>) {
        let mut errors: Vec<ParseErrorDetail> = Vec::new();
        let mut state_stack: Vec<usize> = vec![self.table.start_state];
        let mut node_stack: Vec<ParseNode> = Vec::new();

        let mut input = tokens;
        input.push(ParseToken { kind: "$".to_string(), lexeme: String::new() });
        let mut ip = 0usize;
        // Recuerda la posición de la última vez que entramos en modo pánico SIN que
        // ningún Shift haya consumido input desde entonces. Si volvemos a entrar en
        // pánico exactamente en la misma posición, es que la recuperación anterior
        // (desapilar hasta un estado con acción para el símbolo de sync) llevó a un
        // ε-reduce que no avanza `ip` y vuelve a fallar — un ciclo real sin cota
        // (A10). Forzar el avance de un token rompe el ciclo garantizando progreso.
        let mut last_panic_ip: Option<usize> = None;

        loop {
            let s = *state_stack.last().unwrap();
            // '$' is rejected as a token name at grammar-parse time, so no Shift can
            // ever push `ip` past it — index defensively anyway instead of panicking (A5).
            let current = match input.get(ip) {
                Some(t) => t,
                None => {
                    errors.push(ParseErrorDetail {
                        pos: ip,
                        token: String::new(),
                        msg: "Error interno: se agotó la entrada de forma inesperada.".to_string(),
                    });
                    return (None, errors);
                }
            };
            let a = current.kind.clone();

            match self.table.action.get(&(s, a.clone())) {
                Some(Action::Shift(t)) => {
                    let t = *t;
                    node_stack.push(ParseNode::leaf(current));
                    state_stack.push(t);
                    ip += 1;
                    last_panic_ip = None; // progreso real: se consumió un token
                }
                Some(Action::Reduce { head, body }) => {
                    let head = head.clone();
                    let k = body.len();
                    let split_at = node_stack.len().saturating_sub(k);
                    let children: Vec<ParseNode> = node_stack.drain(split_at..).collect();
                    for _ in 0..k { state_stack.pop(); }
                    let top = *state_stack.last().unwrap();
                    let next_state = match self.table.goto.get(&(top, head.clone())).copied() {
                        Some(ns) => ns,
                        None => {
                            errors.push(ParseErrorDetail {
                                pos: ip,
                                token: a.clone(),
                                msg: format!(
                                    "Error interno: GOTO[I{}, {}] no definido.", top, head
                                ),
                            });
                            return (None, errors);
                        }
                    };
                    let children = if children.is_empty() {
                        vec![ParseNode::epsilon_leaf()]
                    } else { children };
                    node_stack.push(ParseNode::internal(head, children));
                    state_stack.push(next_state);
                }
                Some(Action::Accept) => {
                    return (node_stack.pop(), errors);
                }
                None => {
                    // ── Modo pánico ──────────────────────────────────────────
                    if last_panic_ip == Some(ip) {
                        // Ya entramos en pánico en esta MISMA posición sin haber
                        // consumido ningún token desde entonces: la recuperación
                        // anterior llevó a un ε-reduce que no avanzó `ip` y volvió a
                        // fallar. Forzar el avance rompe el ciclo (A10).
                        errors.push(ParseErrorDetail {
                            pos: ip,
                            token: a.clone(),
                            msg: format!(
                                "Error sintáctico irrecuperable en la posición actual \
                                 (token '{}'); se descarta para evitar un bucle sin fin.",
                                a
                            ),
                        });
                        ip += 1;
                        last_panic_ip = None;
                        if ip >= input.len() {
                            return (None, errors);
                        }
                        continue;
                    }
                    last_panic_ip = Some(ip);

                    errors.push(ParseErrorDetail {
                        pos: ip,
                        token: a.clone(),
                        msg: format_error(s, &a, &self.table),
                    });

                    // 1. Avanzar el input hasta encontrar un símbolo de sincronización
                    while ip < input.len()
                        && !sync.contains(&input[ip].kind.as_str())
                        && input[ip].kind != "$"
                    {
                        ip += 1;
                    }

                    if ip >= input.len() || input[ip].kind == "$" {
                        // No hay punto de recuperación — abortamos
                        return (None, errors);
                    }

                    let sync_kind = input[ip].kind.clone();

                    // 2. Desapilar estados hasta encontrar uno con acción para sync_kind
                    let mut recovered = false;
                    while state_stack.len() > 1 {
                        let top = *state_stack.last().unwrap();
                        if self.table.action.contains_key(&(top, sync_kind.clone())) {
                            recovered = true;
                            break;
                        }
                        state_stack.pop();
                        if !node_stack.is_empty() { node_stack.pop(); }
                    }

                    if !recovered {
                        return (None, errors);
                    }
                    // 3. Continuar desde el punto de sincronización
                }
            }
        }
    }
}

fn format_error(state: usize, token: &str, table: &LRTable) -> String {
    let expected_str = crate::sintactico::tablas::format_expected_tokens(&table.expected_tokens(state));
    format!("Error sintáctico: estado I{}, token '{}'. Esperado: {}",
            state, token, expected_str)
}

/// Imprime la traza del parseo en formato columnar.
pub fn print_trace(trace: &[ParseStep]) {
    println!("{:<40} Acción", "Pila de estados");
    println!("{}", "-".repeat(60));
    for step in trace {
        match step {
            ParseStep::Shift { state, token } => {
                println!("  push I{}  ←  shift '{}'", state, token);
            }
            ParseStep::Reduce { head, body } => {
                println!("  reduce   :  {} → {}", head, crate::sintactico::gramatica::grammar::body_to_string(body));
            }
            ParseStep::Accept => {
                println!("  accept   ");
            }
        }
    }
}
