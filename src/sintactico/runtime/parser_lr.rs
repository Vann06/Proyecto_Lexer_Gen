// Los tres consumidores del motor shift-reduce que viven en la fase
// sintáctica. El bucle en sí NO está acá: vive una sola vez en
// `super::driver`. Acá quedan tres observadores —traza, árbol y árbol con
// recuperación— y las funciones públicas que los envuelven, con las MISMAS
// firmas de siempre para que ningún llamador se entere del cambio.
use crate::sintactico::gramatica::grammar::Symbol;
use crate::sintactico::runtime::driver::{
    self, DriveError, ErrorCause, OnError, ParseObserver,
};
use crate::sintactico::runtime::parse_tree::{ParseNode, ParseToken};
use crate::sintactico::tablas::LRTable;

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

/// Envuelve una lista de kinds en `ParseToken`s sin posición, para los dos
/// caminos que solo reciben nombres de terminal (`parse` y la traza del IDE).
pub(crate) fn tokens_from_kinds(kinds: Vec<String>) -> Vec<ParseToken> {
    kinds
        .into_iter()
        .map(|kind| ParseToken { kind, lexeme: String::new(), line: 0, col: 0 })
        .collect()
}

/// Traduce el corte del driver al `String` de error que devolvían `parse` y
/// `parse_tree`.
fn drive_error_msg(err: &DriveError, table: &LRTable) -> String {
    match err {
        DriveError::Syntax { state, token, .. } => format_error(*state, &token.kind, table),
        DriveError::MissingGoto { top, head, .. } => DriveError::missing_goto_msg(*top, head),
        DriveError::InputExhausted => DriveError::exhausted_msg(),
        DriveError::Unrecovered => "Error sintáctico irrecuperable.".to_string(),
    }
}

// ── Observador 1: solo la traza de pasos ────────────────────────────────────

#[derive(Default)]
struct TraceObserver {
    trace: Vec<ParseStep>,
}

impl ParseObserver for TraceObserver {
    fn on_shift(&mut self, next_state: usize, token: &ParseToken) {
        self.trace.push(ParseStep::Shift { state: next_state, token: token.kind.clone() });
    }
    fn on_reduce(&mut self, head: &str, body: &[Symbol], _goto: usize) {
        self.trace.push(ParseStep::Reduce { head: head.to_string(), body: body.to_vec() });
    }
    fn on_accept(&mut self) {
        self.trace.push(ParseStep::Accept);
    }
}

// ── Observador 2: construcción del árbol ────────────────────────────────────

#[derive(Default)]
struct TreeObserver {
    nodes: Vec<ParseNode>,
}

impl TreeObserver {
    /// Reduce sobre la pila de nodos: saca los `k` hijos y apila el nodo
    /// interno. Una reducción de cuerpo VACÍO deja una hoja ε visible en el
    /// árbol — es la diferencia real entre este observador y el de traza, y
    /// hay que conservarla.
    fn reduce(&mut self, head: &str, k: usize) {
        let split_at = self.nodes.len().saturating_sub(k);
        let children: Vec<ParseNode> = self.nodes.drain(split_at..).collect();
        let children = if children.is_empty() {
            vec![ParseNode::epsilon_leaf()]
        } else {
            children
        };
        self.nodes.push(ParseNode::internal(head.to_string(), children));
    }
}

impl ParseObserver for TreeObserver {
    fn on_shift(&mut self, _next_state: usize, token: &ParseToken) {
        self.nodes.push(ParseNode::leaf(token));
    }
    fn on_reduce(&mut self, head: &str, body: &[Symbol], _goto: usize) {
        self.reduce(head, body.len());
    }
    fn on_discard_state(&mut self) {
        self.nodes.pop();
    }
}

// ── Observador 3: árbol + recuperación en modo pánico ───────────────────────

struct RecoveringObserver<'t> {
    tree: TreeObserver,
    errors: Vec<ParseErrorDetail>,
    table: &'t LRTable,
}

impl<'t> ParseObserver for RecoveringObserver<'t> {
    fn on_shift(&mut self, next_state: usize, token: &ParseToken) {
        self.tree.on_shift(next_state, token);
    }
    fn on_reduce(&mut self, head: &str, body: &[Symbol], goto: usize) {
        self.tree.on_reduce(head, body, goto);
    }
    fn on_discard_state(&mut self) {
        // `TreeObserver::on_discard_state` desapila incondicionalmente; acá la
        // pila puede estar vacía si el error llegó antes de apilar nada.
        if !self.tree.nodes.is_empty() {
            self.tree.nodes.pop();
        }
    }
    fn on_error(
        &mut self,
        cause: ErrorCause,
        state: usize,
        ip: usize,
        token: &ParseToken,
        _table: &LRTable,
    ) -> OnError {
        let msg = match cause {
            ErrorCause::NoAction => format_error(state, &token.kind, self.table),
            ErrorCause::LoopGuard => format!(
                "Error sintáctico irrecuperable en la posición actual \
                 (token '{}'); se descarta para evitar un bucle sin fin.",
                token.kind
            ),
        };
        self.errors.push(ParseErrorDetail { pos: ip, token: token.kind.clone(), msg });
        OnError::Recover
    }
}

impl<'a> LRParser<'a> {
    pub fn new(table: &'a LRTable) -> Self {
        LRParser { table }
    }

    /// Ejecuta shift-reduce. `tokens` debe ser una lista de terminales SIN el $ final.
    /// Devuelve la traza de pasos o un mensaje de error sintáctico.
    pub fn parse(&self, tokens: Vec<String>) -> Result<Vec<ParseStep>, String> {
        let mut obs = TraceObserver::default();
        match driver::drive(self.table, tokens_from_kinds(tokens), &[], &mut obs) {
            Ok(()) => Ok(obs.trace),
            Err(e) => Err(drive_error_msg(&e, self.table)),
        }
    }

    /// Ejecuta shift-reduce y construye el árbol de derivación BOTTOM-UP.
    /// Mantiene una pila paralela de `ParseNode`:
    ///   - Shift token t        → push hoja(t)
    ///   - Reduce A → α (|α|=k) → pop k nodos, push interno(A, esos k nodos)
    ///   - Accept               → la pila contiene la raíz
    pub fn parse_tree(&self, tokens: Vec<ParseToken>) -> Result<ParseNode, String> {
        let mut obs = TreeObserver::default();
        match driver::drive(self.table, tokens, &[], &mut obs) {
            Ok(()) => obs
                .nodes
                .pop()
                .ok_or_else(|| "Error interno: Accept con pila de nodos vacía.".to_string()),
            Err(e) => Err(drive_error_msg(&e, self.table)),
        }
    }

    /// Parseo con recuperación en modo pánico:
    ///   1. Registra el error.
    ///   2. Descarta tokens del input hasta encontrar un símbolo de sincronización.
    ///   3. Desapila estados hasta encontrar uno que acepte ese símbolo.
    ///   4. Retoma el parseo desde ahí.
    ///
    /// Devuelve el árbol (si logró llegar a Accept) y TODOS los errores
    /// encontrados, no solo el primero.
    pub fn parse_recovering_with_pos(
        &self,
        tokens: Vec<ParseToken>,
        sync: &[&str],
    ) -> (Option<ParseNode>, Vec<ParseErrorDetail>) {
        let mut obs = RecoveringObserver {
            tree: TreeObserver::default(),
            errors: Vec::new(),
            table: self.table,
        };

        match driver::drive(self.table, tokens, sync, &mut obs) {
            Ok(()) => (obs.tree.nodes.pop(), obs.errors),
            Err(DriveError::MissingGoto { top, head, ip, token }) => {
                obs.errors.push(ParseErrorDetail {
                    pos: ip,
                    token: token.kind,
                    msg: DriveError::missing_goto_msg(top, &head),
                });
                (None, obs.errors)
            }
            Err(DriveError::InputExhausted) => {
                obs.errors.push(ParseErrorDetail {
                    pos: usize::MAX,
                    token: String::new(),
                    msg: DriveError::exhausted_msg(),
                });
                (None, obs.errors)
            }
            // `Unrecovered` y `Syntax` ya dejaron su diagnóstico en `on_error`.
            Err(_) => (None, obs.errors),
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
