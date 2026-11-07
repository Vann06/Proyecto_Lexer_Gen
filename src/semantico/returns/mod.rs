//! ¿Una función con tipo de retorno devuelve un valor en todos sus caminos?
//!
//! El análisis es conservador por el lado correcto: un camino solo se descarta si
//! la estructura del programa garantiza que no llega al final. Las condiciones no
//! se evalúan, salvo el literal `true`, porque `while (true) { ... return x; }`
//! es un programa válido.
//!
//! Igual que `deadcode`, no conoce palabras reservadas: las construcciones salen
//! de `%flow`, `%return`, `%break`, `%continue` y `%scope`. Una gramática sin
//! `%flow` no se revisa, porque no se sabría qué rama es opcional.

use super::deadcode::flatten_sequence;
use super::scopes::ScopeKind;
use super::spec::SemanticSpec;
use crate::sintactico::gramatica::flow::{FlowDirective, FlowKind, FlowRole};
use crate::sintactico::runtime::parse_tree::ParseNode;

/// Si el cuerpo de una función devuelve un valor en todos sus caminos. Sin `%flow`
/// responde `true`: no hay con qué revisarlo.
pub fn body_always_returns(body: &ParseNode, spec: &SemanticSpec) -> bool {
    spec.flow_shapes.is_empty() || always_returns(body, spec)
}

fn flow_of<'s>(node: &ParseNode, spec: &'s SemanticSpec) -> Option<&'s FlowDirective> {
    spec.flow_shapes.iter().find(|flow| flow.production == node.symbol)
}

fn child<'n>(node: &'n ParseNode, flow: &FlowDirective, role: FlowRole) -> Option<&'n ParseNode> {
    node.children.get(flow.index_of(role)?)
}

/// Una función o clase anidada: sus `return` no son de quien la contiene.
fn is_nested_scope(node: &ParseNode, spec: &SemanticSpec) -> bool {
    spec.scopes
        .iter()
        .any(|scope| scope.production == node.symbol && !matches!(scope.kind, ScopeKind::Block | ScopeKind::Global))
}

fn always_returns(node: &ParseNode, spec: &SemanticSpec) -> bool {
    if spec.returns.iter().any(|rule| rule.production == node.symbol) {
        return true;
    }
    if is_nested_scope(node, spec) {
        return false;
    }
    let Some(flow) = flow_of(node, spec) else {
        // Un bloque o una sentencia: basta con que una de sus partes retorne.
        return node.children.iter().any(|part| always_returns(part, spec));
    };
    let returns = |role| child(node, flow, role).is_some_and(|part| always_returns(part, spec));
    let is_endless = || child(node, flow, FlowRole::Cond).is_some_and(is_true_literal);
    let body = child(node, flow, FlowRole::Body);
    let breaks = || body.is_some_and(|part| has_jump(part, &spec.flow.breaks, spec, stops_a_break));

    match flow.kind {
        FlowKind::If => returns(FlowRole::Then) && returns(FlowRole::Else),
        // El cuerpo puede no ejecutarse: solo cuenta si el ciclo nunca termina.
        FlowKind::While | FlowKind::For => is_endless() && !breaks(),
        // El cuerpo corre al menos una vez, salvo que salte antes de retornar.
        FlowKind::DoWhile => {
            let continues = body.is_some_and(|part| has_jump(part, &spec.flow.continues, spec, stops_a_continue));
            !breaks() && !continues && (returns(FlowRole::Body) || is_endless())
        }
        FlowKind::Foreach => false,
        FlowKind::Switch => switch_always_returns(node, flow, spec),
        FlowKind::Try => returns(FlowRole::Body) && returns(FlowRole::Handler),
        FlowKind::Case | FlowKind::Default | FlowKind::Catch => returns(FlowRole::Body),
    }
}

/// Los `case` caen al siguiente, así que todo camino pasa por el último cuerpo
/// salvo que antes retorne o haga `break`. Sin `default` puede no entrar a ninguno.
fn switch_always_returns(node: &ParseNode, flow: &FlowDirective, spec: &SemanticSpec) -> bool {
    let Some(cases) = child(node, flow, FlowRole::Cases) else {
        return false;
    };
    let (_, members) = flatten_sequence(cases);
    let shape_of = |member: &'_ &ParseNode| member.children.first().and_then(|part| flow_of(part, spec));
    let has_default = members.iter().any(|member| shape_of(member).is_some_and(|f| f.kind == FlowKind::Default));
    has_default
        && !has_jump(cases, &spec.flow.breaks, spec, stops_a_break)
        && members.last().is_some_and(|last| always_returns(last, spec))
}

fn stops_a_break(kind: FlowKind) -> bool {
    stops_a_continue(kind) || kind == FlowKind::Switch
}

fn stops_a_continue(kind: FlowKind) -> bool {
    matches!(kind, FlowKind::While | FlowKind::DoWhile | FlowKind::For | FlowKind::Foreach)
}

/// Si `node` tiene un salto de `jumps` que le pertenece: no cuentan los de un ciclo
/// o `switch` interno, que salen de ese y no de este.
fn has_jump(node: &ParseNode, jumps: &[String], spec: &SemanticSpec, stops: fn(FlowKind) -> bool) -> bool {
    if jumps.iter().any(|jump| jump == &node.symbol) {
        return true;
    }
    if is_nested_scope(node, spec) || flow_of(node, spec).is_some_and(|flow| stops(flow.kind)) {
        return false;
    }
    node.children.iter().any(|part| has_jump(part, jumps, spec, stops))
}

/// `true` escrito tal cual, sin importar cuántos niveles de la gramática lo envuelvan.
fn is_true_literal(node: &ParseNode) -> bool {
    match node.children.as_slice() {
        [] => node.lexeme.as_deref() == Some("true"),
        [only] => is_true_literal(only),
        _ => false,
    }
}
