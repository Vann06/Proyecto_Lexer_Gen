// Configuración declarativa (Fase 15): le dice al walker genérico de
// `super::analyzer` qué producción de UNA gramática concreta declara un
// símbolo y cuál abre un scope nuevo — sin que el walker en sí sepa nada de
// esa gramática. Un `SemanticSpec` nuevo por cada `.yalp` que se reciba, el
// walker no cambia nunca.
use std::collections::HashSet;

use crate::semantico::scopes::ScopeKind;
use crate::semantico::symbols::SymbolKind;
use crate::sintactico::gramatica::grammar::Grammar;

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

impl SemanticSpec {
    /// Construye un `SemanticSpec` a partir de las directivas `%ident`/
    /// `%declare`/`%scope` de un `.yalp` ya parseado — sin esto, cualquier
    /// gramática que quiera análisis semántico tendría que armar su
    /// `SemanticSpec` a mano en Rust en vez de declararlo en el propio
    /// archivo de gramática. `None` si el `.yalp` no trae `%ident`: una
    /// gramática sin esa directiva sigue compilando y parseando igual, solo
    /// que sin análisis semántico (el generador no deja de ser agnóstico a
    /// la gramática por defecto).
    ///
    /// `with_label` se deriva solo: un `%scope` cuya producción TAMBIÉN
    /// aparece en un `%declare` (p.ej. `func_decl` declara Y abre scope) se
    /// etiqueta con el nombre recién declarado; uno que solo abre scope sin
    /// declarar nada (p.ej. `bloque`) no lleva etiqueta.
    pub fn from_grammar(grammar: &Grammar) -> Option<Self> {
        let identifier_token = grammar.ident_token.clone()?;

        let declared_productions: HashSet<&str> =
            grammar.declare_directives.iter().map(|(prod, _)| prod.as_str()).collect();

        let declarations = grammar
            .declare_directives
            .iter()
            .map(|(production, kind)| DeclarationRule {
                production: production.clone(),
                kind: symbol_kind_from_directive(kind),
                name_child: None,
                implicit: false,
            })
            .collect();

        let scopes = grammar
            .scope_directives
            .iter()
            .map(|(production, kind)| ScopeRule {
                production: production.clone(),
                kind: scope_kind_from_directive(kind),
                with_label: declared_productions.contains(production.as_str()),
            })
            .collect();

        Some(SemanticSpec { identifier_token, declarations, scopes })
    }
}

fn symbol_kind_from_directive(kind: &str) -> SymbolKind {
    match kind {
        "variable" => SymbolKind::Variable,
        "parameter" => SymbolKind::Parameter,
        "function" => SymbolKind::Function,
        "class" => SymbolKind::Class,
        other => SymbolKind::Other(other.to_string()),
    }
}

fn scope_kind_from_directive(kind: &str) -> ScopeKind {
    match kind {
        "global" => ScopeKind::Global,
        "class" => ScopeKind::Class,
        "block" => ScopeKind::Block,
        // "function" y cualquier otro valor no reconocido: Function es el
        // scope más común de una declaración con cuerpo propio.
        _ => ScopeKind::Function,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grammar_with_directives(
        ident: Option<&str>,
        declares: &[(&str, &str)],
        scopes: &[(&str, &str)],
    ) -> Grammar {
        Grammar {
            tokens: Default::default(),
            ignores: Default::default(),
            productions: Vec::new(),
            start_symbol: String::new(),
            transformation_log: Vec::new(),
            precedence: Vec::new(),
            ident_token: ident.map(String::from),
            declare_directives: declares.iter().map(|(p, k)| (p.to_string(), k.to_string())).collect(),
            scope_directives: scopes.iter().map(|(p, k)| (p.to_string(), k.to_string())).collect(),
        }
    }

    #[test]
    fn no_ident_directive_means_no_semantic_spec() {
        let g = grammar_with_directives(None, &[], &[]);
        assert!(SemanticSpec::from_grammar(&g).is_none());
    }

    #[test]
    fn declares_and_scopes_translate_kinds_and_derive_with_label() {
        let g = grammar_with_directives(
            Some("ID"),
            &[("func_decl", "function"), ("var_decl", "variable")],
            &[("func_decl", "function"), ("bloque", "block")],
        );
        let spec = SemanticSpec::from_grammar(&g).expect("trae %ident");
        assert_eq!(spec.identifier_token, "ID");

        let func_decl = spec.declarations.iter().find(|r| r.production == "func_decl").unwrap();
        assert_eq!(func_decl.kind, SymbolKind::Function);
        assert!(!func_decl.implicit);
        assert_eq!(func_decl.name_child, None);

        let func_scope = spec.scopes.iter().find(|r| r.production == "func_decl").unwrap();
        assert_eq!(func_scope.kind, ScopeKind::Function);
        assert!(func_scope.with_label, "func_decl también declara: debe llevar etiqueta");

        let bloque_scope = spec.scopes.iter().find(|r| r.production == "bloque").unwrap();
        assert_eq!(bloque_scope.kind, ScopeKind::Block);
        assert!(!bloque_scope.with_label, "bloque no declara nada: sin etiqueta");
    }

    #[test]
    fn unknown_kind_string_falls_back_to_other_or_function() {
        let g = grammar_with_directives(Some("ID"), &[("thing", "widget")], &[("thing", "weird")]);
        let spec = SemanticSpec::from_grammar(&g).unwrap();
        assert_eq!(
            spec.declarations[0].kind,
            SymbolKind::Other("widget".to_string())
        );
        assert_eq!(spec.scopes[0].kind, ScopeKind::Function);
    }
}
