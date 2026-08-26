// Configuración declarativa (Fase 15): le dice al walker genérico de
// `super::analyzer` qué producción de UNA gramática concreta declara un
// símbolo y cuál abre un scope nuevo — sin que el walker en sí sepa nada de
// esa gramática. Un `SemanticSpec` nuevo por cada `.yalp` que se reciba, el
// walker no cambia nunca.
use std::collections::{HashMap, HashSet};

use crate::semantico::scopes::ScopeKind;
use crate::semantico::symbols::SymbolKind;
use crate::semantico::types::{ArithmeticOperator, Type};
use crate::sintactico::gramatica::grammar::Grammar;

#[derive(Default)]
pub struct SemanticSpec {
    /// Token del lexer que representa un identificador (p.ej. "ID"). Toda
    /// hoja con este `symbol` que no haya sido consumida como el nombre de
    /// una `DeclarationRule` se trata como un USO y se busca con `lookup`.
    pub identifier_token: String,
    pub declarations: Vec<DeclarationRule>,
    pub scopes: Vec<ScopeRule>,
    /// Mapea el `symbol` de la hoja terminal de un nodo de tipo (p.ej.
    /// "INT_T") al `Type` semántico que representa (`Type::Int`). Lo usa
    /// `analyzer::resolve_declared_type` cuando una `DeclarationRule` trae
    /// `type_child`. Un terminal que no aparece acá cae en `Type::Unknown`
    /// — no es error, es "no sabemos mapear ese token a un tipo primitivo".
    /// Vacío (default de `from_grammar`, que hoy no trae ninguna directiva
    /// `%type`): ninguna declaración puede usar `type_child`, así que el
    /// walker sigue llamando a `declare()` como antes de este campo existir.
    pub type_tokens: HashMap<String, Type>,
    /// Token de `this` (p.ej. "THIS"). `None`: la gramática no tiene `this`
    /// — una hoja con ese `symbol` nunca aparece, así que este campo no
    /// afecta nada si se deja sin usar.
    pub this_token: Option<String>,
    /// Producciones con acceso a miembros (`obj.miembro`). Suele haber más
    /// de una: la de LECTURA (`primary: primary DOT ID`) y la de ASIGNACIÓN
    /// (`assign_stmt: primary DOT ID ASSIGN expr`). Vacío: la gramática no
    /// tiene `.` como operador — el walker no intenta resolver ningún
    /// acceso a miembro.
    pub member_access: Vec<MemberAccessRule>,
    /// Configuración de instanciación (`new Clase(args)`). `None`: la
    /// gramática no tiene `new`.
    pub instantiation: Option<InstantiationRule>,
    /// Configuración de llamada (`f(args)`, `obj.metodo(args)`). `None`: el
    /// walker no valida aridad ni tipos de ninguna invocación.
    pub call: Option<CallRule>,
    /// Símbolo de la producción recursiva que separa los argumentos por
    /// comas (p.ej. "args" en `args: args COMMA expr | expr`) — lo necesita
    /// `classes::flatten_arg_list` para aplanar una lista de argumentos sin
    /// conocer el nombre de la producción de expresión. Compartido por
    /// `instantiation` y `call`, que reciben ambos un nodo `arg_list`.
    pub args_list_symbol: Option<String>,
    /// Nombre convencional del método que actúa como constructor de una
    /// clase (p.ej. "constructor", estilo JS/TS — un método normal, no una
    /// palabra reservada de la gramática). `None`: ninguna clase tiene
    /// constructor explícito, todas usan el implícito de aridad 0.
    pub constructor_name: Option<String>,
    /// Configuración de asignación a una variable suelta (`x = expr`).
    /// `None`: el walker no chequea tipo ni mutabilidad en asignaciones.
    /// (La asignación a una PROPIEDAD, `obj.attr = expr`, se configura
    /// aparte con `member_access` — es otra producción.)
    pub assign: Option<AssignRule>,
    /// Mapea el token de un operador aritmético binario (p.ej. "PLUS") a la
    /// operación que representa. Un nodo de tres hijos cuyo hijo del medio
    /// sea uno de estos tokens se trata como `izq OP der` y se le calcula el
    /// tipo con `types::resolve_arithmetic`. Vacío: no se tipa ni se valida
    /// ninguna expresión aritmética.
    pub arith_tokens: HashMap<String, ArithmeticOperator>,
    /// Configuración del literal de struct (`Punto { x: 1, y: 2 }`).
    /// `None`: la gramática no tiene literales de struct y el walker no
    /// tipa ni valida ninguno.
    pub struct_literal: Option<StructLiteralRule>,
    /// Símbolo de la producción recursiva que separa los campos de un
    /// literal por comas — el análogo de `args_list_symbol`, y lo consume el
    /// mismo `classes::flatten_arg_list`.
    pub field_list_symbol: Option<String>,
    /// Forma de un campo suelto dentro de un literal (`x: 1`). Se declara
    /// aparte porque el walker necesita reconocer ESE nodo por sí solo —
    /// llega a él sin saber quién es su padre— para excluir la etiqueta del
    /// campo del recorrido y que no se reporte como variable no declarada.
    pub field_init: Option<FieldInitRule>,
    /// Producciones que retornan de una función, una por línea `%return`.
    /// Vacío: no se valida ningún `return` contra el tipo declarado de la
    /// función que lo contiene.
    pub returns: Vec<ReturnRule>,
    /// Reglas de control de flujo declaradas por la gramática.
    pub flow: FlowSpec,
}

/// "La producción `production` asigna el valor de `value_index` a la
/// variable nombrada por el identificador en `target_index`" — p.ej.
/// `assign_stmt: ID ASSIGN expr` con `target_index: 0`, `value_index: 2`.
///
/// Igual que las demás reglas, la alternativa concreta se reconoce por la
/// FORMA: solo dispara si el hijo en `target_index` es una HOJA con el
/// `identifier_token`. Así, la alternativa de asignación a propiedad del
/// mismo head (`assign_stmt: primary DOT ID ASSIGN expr`, cuyo hijo 0 es un
/// nodo interno) no la toma — esa la maneja `member_access`.
pub struct AssignRule {
    pub production: String,
    pub target_index: usize,
    pub value_index: usize,
}

/// Dónde está el nodo de tipo dentro de los hijos DIRECTOS de una
/// producción. `Index` sirve para una forma fija de un solo hijo posible
/// (p.ej. `atom: NEW ID LPAREN arg_list RPAREN`, siempre en la misma
/// posición); `BySymbol` para producciones con varias alternativas donde el
/// nodo de tipo cambia de posición o directamente no aparece (p.ej.
/// `var_decl: let_or_var ID | let_or_var ID COLON tipo | ...` — un índice
/// fijo sería incorrecto para más de una alternativa a la vez).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildLocator {
    Index(usize),
    BySymbol(String),
}

/// "La producción `production` accede a un miembro del valor que precede al
/// `dot_token`" — p.ej. `primary: primary DOT ID` (lectura) o
/// `assign_stmt: primary DOT ID ASSIGN expr` (asignación). El walker
/// distingue una de otra por la forma del nodo, no por configuración: si hay
/// hijos MÁS ALLÁ del nombre del miembro, el último es el valor asignado
/// (ver `analyzer::enter`).
pub struct MemberAccessRule {
    pub production: String,
    pub dot_token: String,
}

/// "La producción `production` tiene una alternativa `new_token ID LPAREN
/// arg_list RPAREN` que instancia una clase" — p.ej.
/// `atom: NEW ID LPAREN arg_list RPAREN`. `class_name_index`/
/// `arg_list_index` son los índices, entre los hijos DIRECTOS de esa
/// alternativa, del ID (nombre de clase) y del nodo `arg_list`.
pub struct InstantiationRule {
    pub production: String,
    pub new_token: String,
    pub class_name_index: usize,
    pub arg_list_index: usize,
}

/// "La producción `production` tiene una alternativa que INVOCA lo que
/// precede al paréntesis de apertura" — p.ej.
/// `primary: primary LPAREN arg_list RPAREN`, que cubre por igual una
/// llamada a función libre (`f(1)`) y a método (`obj.m(1)`), porque el
/// llamado es simplemente la subexpresión de la izquierda.
///
/// La alternativa concreta se reconoce por la FORMA, no por configuración
/// extra: de las varias alternativas del mismo head (`primary: atom` /
/// `primary DOT ID` / `primary LPAREN arg_list RPAREN`), solo la de llamada
/// trae `open_paren_token` como hijo directo — mismo criterio que usa
/// `InstantiationRule` con su `new_token`.
pub struct CallRule {
    pub production: String,
    pub open_paren_token: String,
    pub callee_index: usize,
    pub arg_list_index: usize,
}

/// "La producción `production` tiene una alternativa que construye un valor
/// de struct con campos nombrados" — p.ej.
/// `atom: ID LBRACE field_inits RBRACE`, con `type_name_index: 0` y
/// `field_list_index: 2`.
///
/// Igual que `InstantiationRule` y `CallRule`, la alternativa concreta se
/// reconoce por la FORMA: de las varias alternativas de `atom`, solo esta
/// trae `open_brace_token` como hijo directo.
pub struct StructLiteralRule {
    pub production: String,
    pub open_brace_token: String,
    pub type_name_index: usize,
    pub field_list_index: usize,
}

/// "La producción `production` es un campo de un literal de struct, con su
/// nombre en `name_index` y su valor en `value_index`" — p.ej.
/// `field_init: ID COLON expr` con `0` y `2`.
pub struct FieldInitRule {
    pub production: String,
    pub name_index: usize,
    pub value_index: usize,
}

/// "La producción `production` retorna de la función que la contiene, y el
/// valor retornado —si lo hay— es su hijo `value_child`" — p.ej.
/// `return_stmt: RETURN expr | RETURN`.
///
/// `value_child` es un `ChildLocator` y en la práctica siempre `BySymbol`:
/// las dos alternativas tienen distinta aridad, así que un índice fijo no
/// sirve. Buscar por símbolo devuelve `None` para la alternativa sin valor,
/// y esa ausencia es exactamente la señal de "retorno vacío" — el mismo
/// truco que usa `DeclarationRule::init_child` para un `var_decl` con y sin
/// inicializador.
pub struct ReturnRule {
    pub production: String,
    pub value_child: ChildLocator,
}

#[derive(Debug, Default)]
pub struct FlowSpec {
    pub conditions: Vec<ConditionRule>,
    pub loops: Vec<String>,
    pub breaks: Vec<String>,
    pub continues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionRule {
    pub production: String,
    pub condition_child: ChildLocator,
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
    /// Dónde está, entre los hijos DIRECTOS de esta producción, el nodo que
    /// representa el TIPO declarado. `None` (default): esta declaración no
    /// lleva tipo — el walker sigue usando `SymbolTable::declare` sin tocar
    /// `ty`, igual que antes de que este campo existiera. `Some(locator)`:
    /// el walker resuelve ese subárbol con `analyzer::resolve_declared_type`
    /// (usando `SemanticSpec::type_tokens`) y declara con `declare_typed` en
    /// su lugar, dejando `ty` poblado — incluido el caso de un nombre de
    /// clase como tipo (`resolve_declared_type` lo resuelve a
    /// `Type::Named`), porque el walker excluye ese hijo de la recursión
    /// genérica vía `Flow::SkipChildIndices` (ya no se re-visita como uso).
    pub type_child: Option<ChildLocator>,
    /// Dónde está el nodo de la expresión INICIALIZADORA (`let x: T = expr`,
    /// el `expr`). `None`: no se chequea el inicializador contra el tipo
    /// declarado. `Some(locator)`: el walker resuelve su tipo con
    /// `classes::resolve_expr_type` y se lo pasa a `declare_typed`, que lo
    /// valida con la misma tabla de coerciones de siempre. A diferencia de
    /// `type_child`, este hijo NO se excluye del recorrido: es una expresión
    /// real, y los identificadores que use tienen que seguir validándose.
    pub init_child: Option<ChildLocator>,
    /// `true` para una declaración de CONSTANTE (`const`): el símbolo se
    /// declara con `mutable: false`, lo que hace que `declare_typed` exija
    /// inicializador y que cualquier asignación posterior sea un error.
    pub immutable: bool,
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
            .map(|(production, kind)| {
                // `%type_of` es por-producción, igual que `%declare`/`%scope`
                // — si esta producción no tiene una línea `%type_of`, queda
                // `None` (comportamiento idéntico a antes de que existiera).
                let type_child = grammar
                    .type_of_directives
                    .iter()
                    .find(|(p, _)| p == production)
                    .map(|(_, symbol)| ChildLocator::BySymbol(symbol.clone()));

                let init_child = grammar
                    .init_of_directives
                    .iter()
                    .find(|(p, _)| p == production)
                    .map(|(_, symbol)| ChildLocator::BySymbol(symbol.clone()));

                DeclarationRule {
                    production: production.clone(),
                    kind: symbol_kind_from_directive(kind),
                    name_child: None,
                    implicit: false,
                    type_child,
                    init_child,
                    immutable: grammar.immutable_directives.iter().any(|p| p == production),
                }
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

        let type_tokens = grammar
            .type_token_directives
            .iter()
            .map(|(token, kind)| (token.clone(), type_from_directive(kind)))
            .collect();

        let member_access = grammar
            .member_access_directives
            .iter()
            .map(|(production, dot_token)| MemberAccessRule {
                production: production.clone(),
                dot_token: dot_token.clone(),
            })
            .collect();

        let instantiation =
            grammar.instantiation.clone().map(|(production, new_token, class_name_index, arg_list_index)| {
                InstantiationRule { production, new_token, class_name_index, arg_list_index }
            });

        let call = grammar.call.clone().map(|(production, open_paren_token, callee_index, arg_list_index)| CallRule {
            production,
            open_paren_token,
            callee_index,
            arg_list_index,
        });

        Some(SemanticSpec {
            identifier_token,
            declarations,
            scopes,
            type_tokens,
            this_token: grammar.this_token.clone(),
            member_access,
            instantiation,
            call,
            args_list_symbol: grammar.arg_list_symbol.clone(),
            constructor_name: grammar.constructor_name.clone(),
            assign: grammar.assign.clone().map(|(production, target_index, value_index)| AssignRule {
                production,
                target_index,
                value_index,
            }),
            arith_tokens: grammar
                .arith_directives
                .iter()
                .filter_map(|(token, op)| arith_operator_from_directive(op).map(|o| (token.clone(), o)))
                .collect(),
            struct_literal: grammar.struct_literal.clone().map(
                |(production, open_brace_token, type_name_index, field_list_index)| StructLiteralRule {
                    production,
                    open_brace_token,
                    type_name_index,
                    field_list_index,
                },
            ),
            field_list_symbol: grammar.field_list_symbol.clone(),
            field_init: grammar.field_init.clone().map(|(production, name_index, value_index)| FieldInitRule {
                production,
                name_index,
                value_index,
            }),
            returns: grammar
                .return_directives
                .iter()
                .map(|(production, value_symbol)| ReturnRule {
                    production: production.clone(),
                    value_child: ChildLocator::BySymbol(value_symbol.clone()),
                })
                .collect(),
            flow: FlowSpec {
                conditions: grammar
                    .condition_directives
                    .iter()
                    .map(|(production, locator)| ConditionRule {
                        production: production.clone(),
                        condition_child: child_locator_from_directive(locator),
                    })
                    .collect(),
                loops: grammar.loop_directives.clone(),
                breaks: grammar.break_directives.clone(),
                continues: grammar.continue_directives.clone(),
            },
        })
    }
}

fn child_locator_from_directive(value: &str) -> ChildLocator {
    value
        .parse::<usize>()
        .map(ChildLocator::Index)
        .unwrap_or_else(|_| ChildLocator::BySymbol(value.to_string()))
}

fn symbol_kind_from_directive(kind: &str) -> SymbolKind {
    match kind {
        "variable" => SymbolKind::Variable,
        "parameter" => SymbolKind::Parameter,
        "function" => SymbolKind::Function,
        "class" => SymbolKind::Class,
        "struct" => SymbolKind::Struct,
        other => SymbolKind::Other(other.to_string()),
    }
}

/// A diferencia de las demás traducciones de directiva, esta devuelve
/// `Option`: un operador no reconocido NO se mapea a un default arbitrario
/// (no existe uno razonable), simplemente esa línea `%arith` se ignora y ese
/// token no se trata como aritmético.
fn arith_operator_from_directive(op: &str) -> Option<ArithmeticOperator> {
    match op {
        "add" => Some(ArithmeticOperator::Add),
        "subtract" => Some(ArithmeticOperator::Subtract),
        "multiply" => Some(ArithmeticOperator::Multiply),
        "divide" => Some(ArithmeticOperator::Divide),
        _ => None,
    }
}

fn type_from_directive(kind: &str) -> Type {
    match kind {
        "bool" => Type::Bool,
        "integer" => Type::Int,
        "float" => Type::Float,
        "string" => Type::Str,
        "void" => Type::Void,
        _ => Type::Unknown,
    }
}

fn scope_kind_from_directive(kind: &str) -> ScopeKind {
    match kind {
        "global" => ScopeKind::Global,
        "class" => ScopeKind::Class,
        "struct" => ScopeKind::Struct,
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
        grammar_with_all_directives(ident, declares, scopes, &[], &[])
    }

    fn grammar_with_all_directives(
        ident: Option<&str>,
        declares: &[(&str, &str)],
        scopes: &[(&str, &str)],
        type_children: &[(&str, &str)],
        type_tokens: &[(&str, &str)],
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
            type_of_directives: type_children.iter().map(|(p, c)| (p.to_string(), c.to_string())).collect(),
            type_token_directives: type_tokens.iter().map(|(t, n)| (t.to_string(), n.to_string())).collect(),
            this_token: None,
            member_access_directives: Vec::new(),
            instantiation: None,
            call: None,
            arg_list_symbol: None,
            constructor_name: None,
            init_of_directives: Vec::new(),
            immutable_directives: Vec::new(),
            assign: None,
            arith_directives: Vec::new(),
            struct_literal: None,
            field_list_symbol: None,
            field_init: None,
            return_directives: Vec::new(),
            condition_directives: Vec::new(),
            loop_directives: Vec::new(),
            break_directives: Vec::new(),
            continue_directives: Vec::new(),
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
        assert_eq!(func_decl.type_child, None, "el .yalp no trae directiva de tipo todavía");
        assert!(spec.type_tokens.is_empty());

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

    #[test]
    fn from_grammar_collects_type_of_and_type_token_directives() {
        // %type_of / %type_token quedan en `DeclarationRule::type_child` (por
        // producción) y `SemanticSpec::type_tokens` (mapa global de tokens) —
        // la resolución real del subárbol de tipo vive en `analyzer`, no acá.
        let g = grammar_with_all_directives(
            Some("ID"),
            &[("var_decl", "variable"), ("param", "parameter")],
            &[],
            &[("var_decl", "tipo"), ("param", "tipo")],
            &[("BOOL_T", "bool"), ("INT_T", "integer"), ("STR_T", "string")],
        );
        let spec = SemanticSpec::from_grammar(&g).unwrap();

        let var_decl = spec.declarations.iter().find(|r| r.production == "var_decl").unwrap();
        assert_eq!(var_decl.type_child, Some(ChildLocator::BySymbol("tipo".to_string())));
        let param = spec.declarations.iter().find(|r| r.production == "param").unwrap();
        assert_eq!(param.type_child, Some(ChildLocator::BySymbol("tipo".to_string())));

        assert_eq!(spec.type_tokens.get("BOOL_T"), Some(&Type::Bool));
        assert_eq!(spec.type_tokens.get("INT_T"), Some(&Type::Int));
        assert_eq!(spec.type_tokens.get("STR_T"), Some(&Type::Str));
    }
}
