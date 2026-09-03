// Tests de CARACTERIZACIÓN del motor shift-reduce.
//
// Fijan el comportamiento observable de los cuatro caminos del parser ANTES de
// unificarlos detrás de un solo driver. No describen lo que "debería" pasar:
// describen lo que pasa hoy, para que el refactor no pueda cambiarlo sin que
// algo se ponga rojo.
//
// El foco está en la recuperación de errores (modo pánico), que era el camino
// menos cubierto de forma directa —solo se ejercitaba de rebote a través del
// pipeline HTTP— y el más delicado de los cuatro: es el único que altera la
// pila por su cuenta.
use lexer_generator::sintactico::automatas::lalr::merge_by_core;
use lexer_generator::sintactico::automatas::lr1::LR1Automaton;
use lexer_generator::sintactico::gramatica::first::calculate_first;
use lexer_generator::sintactico::gramatica::grammar::Grammar;
use lexer_generator::sintactico::runtime::parse_tree::ParseToken;
use lexer_generator::sintactico::runtime::parser_lr::{LRParser, ParseStep};
use lexer_generator::sintactico::tablas::LRTable;

/// Gramática mínima de sentencias separadas por `;` — permite provocar varios
/// errores independientes y comprobar que la recuperación sigue después de
/// cada uno.
const GRAMMAR: &str = "%token ID NUM PLUS SEMI LPAREN RPAREN\n\
                       %%\n\
                       programa : lista ;\n\
                       lista : lista sent | sent ;\n\
                       sent : expr SEMI ;\n\
                       expr : expr PLUS term | term ;\n\
                       term : ID | NUM | LPAREN expr RPAREN ;\n";

fn table() -> LRTable {
    let grammar = Grammar::parse_for_lr_from_str(GRAMMAR).expect("gramática válida");
    let first = calculate_first(&grammar);
    let lalr = merge_by_core(LR1Automaton::build(&grammar, &first));
    LRTable::build_from_lalr(&lalr, &grammar)
}

fn toks(kinds: &[&str]) -> Vec<ParseToken> {
    kinds
        .iter()
        .enumerate()
        .map(|(i, k)| ParseToken {
            kind: k.to_string(),
            lexeme: k.to_lowercase(),
            line: 1,
            col: i + 1,
        })
        .collect()
}

const SYNC: [&str; 6] = ["ID", "NUM", "PLUS", "SEMI", "LPAREN", "RPAREN"];

// ── Camino 1: parse (solo traza) ────────────────────────────────────────────

#[test]
fn parse_emits_shift_reduce_accept_in_order() {
    let t = table();
    let trace = LRParser::new(&t)
        .parse(vec!["ID".into(), "SEMI".into()])
        .expect("entrada válida");

    // La traza siempre termina en Accept y empieza con un Shift.
    assert!(matches!(trace.first(), Some(ParseStep::Shift { .. })));
    assert!(matches!(trace.last(), Some(ParseStep::Accept)));

    let shifts = trace.iter().filter(|s| matches!(s, ParseStep::Shift { .. })).count();
    assert_eq!(shifts, 2, "se consumen exactamente los dos tokens: {trace:?}");
    assert!(trace.iter().any(|s| matches!(s, ParseStep::Reduce { .. })));
}

#[test]
fn parse_reports_the_offending_token_on_error() {
    let t = table();
    let err = LRParser::new(&t)
        .parse(vec!["PLUS".into()])
        .expect_err("PLUS no puede empezar una sentencia");
    assert!(err.contains("PLUS"), "el mensaje debe nombrar el token: {err}");
}

// ── Camino 2: parse_tree ────────────────────────────────────────────────────

#[test]
fn parse_tree_builds_the_root_and_keeps_token_positions() {
    let t = table();
    let tree = LRParser::new(&t)
        .parse_tree(toks(&["ID", "SEMI"]))
        .expect("entrada válida");

    assert_eq!(tree.symbol, "programa");

    // Las hojas conservan la posición que traía cada token.
    fn leaves(n: &lexer_generator::sintactico::runtime::parse_tree::ParseNode, out: &mut Vec<(String, usize)>) {
        if n.children.is_empty() {
            out.push((n.symbol.clone(), n.col));
        }
        for c in &n.children {
            leaves(c, out);
        }
    }
    let mut found = Vec::new();
    leaves(&tree, &mut found);
    assert_eq!(found, vec![("ID".to_string(), 1), ("SEMI".to_string(), 2)]);
}

#[test]
fn parse_tree_fails_on_invalid_input_without_panicking() {
    let t = table();
    assert!(LRParser::new(&t).parse_tree(toks(&["PLUS"])).is_err());
}

// ── Camino 3: parse_recovering_with_pos (modo pánico) ───────────────────────

#[test]
fn recovery_reports_every_error_not_just_the_first() {
    let t = table();
    // Dos sentencias rotas: sobra un PLUS en cada una.
    let (_, errors) = LRParser::new(&t)
        .parse_recovering_with_pos(toks(&["ID", "PLUS", "PLUS", "SEMI", "ID", "PLUS", "PLUS", "SEMI"]), &SYNC);

    assert!(
        errors.len() >= 2,
        "el modo pánico debe reportar los DOS errores en una pasada, no solo el primero: {errors:?}"
    );
    // Cada error apunta a una posición real de la entrada y nombra un token.
    for e in &errors {
        assert!(e.pos < 8, "posición dentro de la entrada: {e:?}");
        assert!(!e.token.is_empty(), "el error nombra el token: {e:?}");
        assert!(!e.msg.is_empty());
    }
}

#[test]
fn recovery_returns_a_tree_when_it_can_resynchronize() {
    let t = table();
    // Un solo error al principio; el resto de la entrada es válido.
    let (tree, errors) = LRParser::new(&t)
        .parse_recovering_with_pos(toks(&["PLUS", "ID", "SEMI"]), &SYNC);
    assert!(!errors.is_empty(), "debe reportar el PLUS sobrante");
    // Si logró resincronizar, el árbol existe; ese es el contrato actual.
    if let Some(tree) = tree {
        assert_eq!(tree.symbol, "programa");
    }
}

#[test]
fn recovery_terminates_on_pathological_input() {
    // La guarda contra ciclos: una entrada que solo tiene tokens que nunca
    // sincronizan no debe colgar el parser ni crecer sin fin. Sin esa guarda
    // este test no termina.
    let t = table();
    let (_, errors) = LRParser::new(&t)
        .parse_recovering_with_pos(toks(&["PLUS", "PLUS", "PLUS", "PLUS"]), &SYNC);
    assert!(!errors.is_empty());
    assert!(errors.len() <= 16, "no debe acumular errores sin cota: {}", errors.len());
}

#[test]
fn recovery_on_clean_input_reports_nothing_and_matches_parse_tree() {
    let t = table();
    let p = LRParser::new(&t);
    let (tree, errors) = p.parse_recovering_with_pos(toks(&["ID", "PLUS", "NUM", "SEMI"]), &SYNC);

    assert!(errors.is_empty(), "entrada válida: sin errores — {errors:?}");
    // Y el árbol debe ser el MISMO que construye el camino sin recuperación:
    // los dos motores tienen que coincidir sobre entradas correctas.
    let direct = p.parse_tree(toks(&["ID", "PLUS", "NUM", "SEMI"])).expect("válida");
    assert_eq!(
        format!("{:?}", tree.expect("hay árbol")),
        format!("{:?}", direct),
        "recuperación y parseo directo deben coincidir sobre entrada válida"
    );
}

// ── Invariante entre caminos ────────────────────────────────────────────────

#[test]
fn empty_body_reductions_produce_an_epsilon_leaf() {
    // Gramática con una producción vacía: el árbol debe mostrar la ε
    // explícitamente. Es una diferencia real entre los motores (solo los que
    // construyen árbol la aplican) y hay que conservarla.
    let g = "%token A\n%%\nS : A opt ;\nopt : A | ;\n";
    let grammar = Grammar::parse_for_lr_from_str(g).expect("válida");
    let first = calculate_first(&grammar);
    let lalr = merge_by_core(LR1Automaton::build(&grammar, &first));
    let t = LRTable::build_from_lalr(&lalr, &grammar);

    let tree = LRParser::new(&t).parse_tree(toks(&["A"])).expect("A seguido de opt vacío");
    let dot = lexer_generator::sintactico::runtime::parse_tree::to_dot(&tree);
    assert!(
        dot.contains('ε'),
        "una reducción de cuerpo vacío debe dejar una hoja ε visible: {dot}"
    );
}
