// Configuración declarativa (Fase 15): le dice al walker genérico de
// `super::analyzer` qué producción de UNA gramática concreta declara un
// símbolo y cuál abre un scope nuevo — sin que el walker en sí sepa nada de
// esa gramática. Un `SemanticSpec` nuevo por cada `.yalp` que se reciba, el
// walker no cambia nunca.
use crate::semantico::scopes::ScopeKind;
use crate::semantico::symbols::SymbolKind;

pub struct SemanticSpec {
    /// Token del lexer que representa un identificador (p.ej. "ID"). Toda
    /// hoja con este `symbol` que no haya sido consumida como el nombre de
    /// una `DeclarationRule` se trata como un USO y se busca con `lookup`.
    pub identifier_token: String,
    pub declarations: Vec<DeclarationRule>,
    pub scopes: Vec<ScopeRule>,
}

/// "La producción `production` declara un símbolo."
pub struct DeclarationRule {
    /// Nombre del no-terminal (el `head` de la producción, tal como
    /// aparece en `ParseNode::symbol` tras la reducción).
    pub production: String,
    pub kind: SymbolKind,
    /// `None` (recomendado): usa el PRIMER hijo directo cuyo `symbol` sea
    /// `identifier_token`. Soporta sin casos especiales tanto producciones
    /// de forma fija (`var_decl: tipo ID`, el ID siempre en la posición 1)
    /// como listas recursivas donde el ID cambia de posición según la
    /// alternativa (`param_list: param_list COMMA ID | ID`) — y descarta
    /// solo las alternativas de un mismo head que no declaran nada (p.ej.
    /// `stmt: ID ASSIGN expr | RETURN expr | expr`: si no hay un hijo ID
    /// directo, la regla simplemente no dispara para esa reducción).
    /// `Some(i)` para desambiguar si una producción tuviera más de un hijo
    /// directo con ese `symbol`.
    pub name_child: Option<usize>,
    /// `false` (default): declarar un nombre que ya existe en el scope
    /// ACTUAL es un error real — la declaración es explícita en la
    /// gramática (`var_decl`, `param`), así que redeclararla es un bug del
    /// programa de entrada.
    /// `true`: para gramáticas donde "declarar" es implícito en la primera
    /// asignación (sin `var_decl` separado). Si el nombre YA es visible en
    /// algún scope, esto es una reasignación normal — no declara de nuevo,
    /// no es error. Si no existe en ningún lado, la primera asignación lo
    /// declara ahí mismo.
    pub implicit: bool,
}

/// "La producción `production` abre un scope nuevo mientras se recorren
/// sus hijos (excepto el consumido por una `DeclarationRule`, si la hay)."
pub struct ScopeRule {
    pub production: String,
    pub kind: ScopeKind,
    /// Si `true`, usa el mismo auto-hallazgo de `DeclarationRule::name_child
    /// == None` para etiquetar el scope (p.ej. el nombre de la función o
    /// clase) — solo afecta la lectura de `dump()`, no la semántica.
    pub with_label: bool,
}
