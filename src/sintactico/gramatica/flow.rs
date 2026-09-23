// Vocabulario de la directiva `%flow`: qué construcciones de control de flujo
// existen y qué partes (roles) tiene cada una.
//
// `%flow` le dice a la fase de código intermedio, para una producción
// concreta, qué hijo es la condición, cuál el cuerpo, cuál el `else`, etc.
// — lo que necesita para emitir etiquetas y saltos (§6.6–6.7 del libro del
// dragón). Sin esto el generador tendría que conocer los nombres de
// producción de una gramática concreta, y el proyecto no puede asumir
// ninguna.
//
//     %flow if      if_stmt  cond=2 then=4 else=6
//     %flow while   while_stmt cond=2 body=4
//
// Vive en `sintactico::gramatica` y no en `intermedio` porque `Grammar` lo
// valida AL LEER el `.yalp` (un `%flow` mal escrito generaría saltos
// equivocados en silencio, así que es un error de la gramática, no una
// advertencia) y la capa sintáctica no depende de las fases posteriores.
// `intermedio::spec` reutiliza estos mismos tipos.

use std::fmt;

/// El tipo de construcción que describe un `%flow`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowKind {
    If,
    While,
    DoWhile,
    For,
    Foreach,
    Switch,
    Case,
    Default,
    /// `try { cuerpo } <manejador>`: el manejador es la construcción `catch`.
    Try,
    /// `catch (var) { cuerpo }`.
    Catch,
}

/// Una parte con nombre de una construcción de control de flujo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowRole {
    /// Condición de un `if`/`while`/`do_while`/`for`.
    Cond,
    /// Rama verdadera de un `if`.
    Then,
    /// Rama falsa de un `if` (un `bloque` u otro `if` encadenado).
    Else,
    /// Cuerpo de un bucle o de un `case`/`default`.
    Body,
    /// Inicialización de un `for`.
    Init,
    /// Paso de un `for`.
    Update,
    /// Variable de un `foreach`.
    Var,
    /// Colección que recorre un `foreach`.
    Iter,
    /// Discriminante de un `switch`.
    Disc,
    /// La lista de `case`/`default` de un `switch`.
    Cases,
    /// Valor de un `case`.
    Value,
    /// El `catch` de un `try`.
    Handler,
}

impl FlowKind {
    pub fn from_directive(name: &str) -> Option<Self> {
        Some(match name {
            "if" => FlowKind::If,
            "while" => FlowKind::While,
            "do_while" => FlowKind::DoWhile,
            "for" => FlowKind::For,
            "foreach" => FlowKind::Foreach,
            "switch" => FlowKind::Switch,
            "case" => FlowKind::Case,
            "default" => FlowKind::Default,
            "try" => FlowKind::Try,
            "catch" => FlowKind::Catch,
            _ => return None,
        })
    }

    /// Roles que TODA alternativa de la producción debe traer.
    pub fn required_roles(self) -> &'static [FlowRole] {
        use FlowRole::*;
        match self {
            FlowKind::If => &[Cond, Then],
            FlowKind::While => &[Cond, Body],
            FlowKind::DoWhile => &[Body, Cond],
            FlowKind::For => &[Cond, Body],
            FlowKind::Foreach => &[Var, Iter, Body],
            FlowKind::Switch => &[Disc, Cases],
            FlowKind::Case => &[Value, Body],
            FlowKind::Default => &[Body],
            FlowKind::Try => &[Body, Handler],
            FlowKind::Catch => &[Var, Body],
        }
    }

    /// Roles que pueden faltar en alguna alternativa: su ausencia ES el dato
    /// (un `if` sin `else`, un `for` sin inicialización).
    pub fn optional_roles(self) -> &'static [FlowRole] {
        use FlowRole::*;
        match self {
            FlowKind::If => &[Else],
            FlowKind::For => &[Init, Update],
            _ => &[],
        }
    }

    pub fn accepts(self, role: FlowRole) -> bool {
        self.required_roles().contains(&role) || self.optional_roles().contains(&role)
    }
}

impl FlowRole {
    pub fn from_directive(name: &str) -> Option<Self> {
        Some(match name {
            "cond" => FlowRole::Cond,
            "then" => FlowRole::Then,
            "else" => FlowRole::Else,
            "body" => FlowRole::Body,
            "init" => FlowRole::Init,
            "update" => FlowRole::Update,
            "var" => FlowRole::Var,
            "iter" => FlowRole::Iter,
            "disc" => FlowRole::Disc,
            "cases" => FlowRole::Cases,
            "value" => FlowRole::Value,
            "handler" => FlowRole::Handler,
            _ => return None,
        })
    }
}

impl fmt::Display for FlowKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            FlowKind::If => "if",
            FlowKind::While => "while",
            FlowKind::DoWhile => "do_while",
            FlowKind::For => "for",
            FlowKind::Foreach => "foreach",
            FlowKind::Switch => "switch",
            FlowKind::Case => "case",
            FlowKind::Default => "default",
            FlowKind::Try => "try",
            FlowKind::Catch => "catch",
        })
    }
}

impl fmt::Display for FlowRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            FlowRole::Cond => "cond",
            FlowRole::Then => "then",
            FlowRole::Else => "else",
            FlowRole::Body => "body",
            FlowRole::Init => "init",
            FlowRole::Update => "update",
            FlowRole::Var => "var",
            FlowRole::Iter => "iter",
            FlowRole::Disc => "disc",
            FlowRole::Cases => "cases",
            FlowRole::Value => "value",
            FlowRole::Handler => "handler",
        })
    }
}

/// Una línea `%flow` ya validada contra el vocabulario (no todavía contra
/// las producciones — eso lo hace `Grammar` al terminar de leer el archivo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowDirective {
    pub kind: FlowKind,
    pub production: String,
    pub roles: Vec<(FlowRole, usize)>,
}

impl FlowDirective {
    /// Lee el resto de una línea `%flow` (lo que sigue a la palabra `%flow`).
    pub fn parse(rest: &str) -> Result<Self, String> {
        let line = format!("%flow {}", rest.trim());
        let err = |msg: String| Err(format!("Error en directiva `{line}`: {msg}"));

        let mut parts = rest.split_whitespace();
        let (Some(kind_name), Some(production)) = (parts.next(), parts.next()) else {
            return err("se esperaba `%flow <tipo> <producción> rol=índice ...`.".to_string());
        };
        let Some(kind) = FlowKind::from_directive(kind_name) else {
            return err(format!(
                "tipo desconocido '{kind_name}' (válidos: if, while, do_while, for, foreach, switch, case, default, try, catch)."
            ));
        };

        let mut roles: Vec<(FlowRole, usize)> = Vec::new();
        for part in parts {
            let Some((role_name, index)) = part.split_once('=') else {
                return err(format!("'{part}' no tiene la forma rol=índice."));
            };
            let Some(role) = FlowRole::from_directive(role_name) else {
                return err(format!("rol desconocido '{role_name}'."));
            };
            if !kind.accepts(role) {
                return err(format!("un `{kind}` no tiene rol '{role}'."));
            }
            let Ok(index) = index.parse::<usize>() else {
                return err(format!("el índice de '{role_name}' no es un número: '{index}'."));
            };
            if roles.iter().any(|(r, _)| *r == role) {
                return err(format!("el rol '{role}' aparece dos veces."));
            }
            roles.push((role, index));
        }

        for required in kind.required_roles() {
            if !roles.iter().any(|(r, _)| r == required) {
                return err(format!("a un `{kind}` le falta el rol obligatorio '{required}'."));
            }
        }

        Ok(FlowDirective { kind, production: production.to_string(), roles })
    }

    pub fn index_of(&self, role: FlowRole) -> Option<usize> {
        self.roles.iter().find(|(r, _)| *r == role).map(|(_, i)| *i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_un_if_con_else_opcional() {
        let d = FlowDirective::parse("if if_stmt cond=2 then=4 else=6").unwrap();
        assert_eq!(d.kind, FlowKind::If);
        assert_eq!(d.production, "if_stmt");
        assert_eq!(d.index_of(FlowRole::Cond), Some(2));
        assert_eq!(d.index_of(FlowRole::Else), Some(6));
        assert_eq!(d.index_of(FlowRole::Body), None);
    }

    #[test]
    fn un_tipo_o_un_rol_desconocido_es_un_error() {
        assert!(FlowDirective::parse("loop while_stmt cond=2 body=4").unwrap_err().contains("tipo desconocido"));
        assert!(FlowDirective::parse("while while_stmt cond=2 cuerpo=4").unwrap_err().contains("rol desconocido"));
    }

    #[test]
    fn un_rol_que_no_corresponde_al_tipo_es_un_error() {
        assert!(FlowDirective::parse("while while_stmt cond=2 body=4 else=6").unwrap_err().contains("no tiene rol"));
    }

    #[test]
    fn falta_un_rol_obligatorio() {
        let e = FlowDirective::parse("while while_stmt cond=2").unwrap_err();
        assert!(e.contains("obligatorio 'body'"), "{e}");
    }

    #[test]
    fn try_exige_su_manejador_y_catch_su_variable() {
        let e = FlowDirective::parse("try try_stmt body=1").unwrap_err();
        assert!(e.contains("obligatorio 'handler'"), "{e}");
        let c = FlowDirective::parse("catch catch_clause var=2 body=4").unwrap();
        assert_eq!(c.kind, FlowKind::Catch);
        assert_eq!(c.index_of(FlowRole::Var), Some(2));
    }

    #[test]
    fn indice_mal_escrito_o_rol_repetido() {
        assert!(FlowDirective::parse("while while_stmt cond=x body=4").unwrap_err().contains("no es un número"));
        assert!(FlowDirective::parse("while while_stmt cond=2 cond=3 body=4").unwrap_err().contains("dos veces"));
    }
}
