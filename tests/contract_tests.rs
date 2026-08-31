// Tests de CONTRATO entre fases.
//
// El resto de la suite comprueba RESULTADOS: que tal programa produzca tal
// diagnóstico. Eso es una consecuencia muy indirecta de que el stream de
// tokens, el árbol y la tabla de símbolos estén bien: si una fase le entregara
// algo sutilmente mal a la siguiente y el diagnóstico final igual saliera,
// nada se pondría rojo.
//
// Estos tests fijan la ENTREGA de cada frontera, para que romper un contrato
// falle acá y no tres fases más adelante. Importa especialmente la última
// sección: la fase 16 (código intermedio) va a consumir justo la entrega que
// menos cubierta estaba.
use lexer_generator::api;
use lexer_generator::semantico::analyzer::analyze;
use lexer_generator::semantico::spec::SemanticSpec;
use lexer_generator::semantico::symbols::{SymbolKind, SymbolTable};
use lexer_generator::semantico::types::Type;
use lexer_generator::sintactico::automatas::lalr::merge_by_core;
use lexer_generator::sintactico::automatas::lr1::LR1Automaton;
use lexer_generator::sintactico::gramatica::first::calculate_first;
use lexer_generator::sintactico::gramatica::grammar::Grammar;
use lexer_generator::sintactico::runtime::parse_tree::{to_dot, ParseNode, ParseToken};
use lexer_generator::sintactico::runtime::parser_lr::LRParser;
use lexer_generator::sintactico::tablas::LRTable;
use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("no se pudo leer {path}: {e}"))
}

// ════════════════════════════════════════════════════════════════════════════
// FRONTERA 1 · Léxico → Sintáctico
//
// Lo que el parser recibe es el `token_map`: una lista de (kind, lexema,
// línea, columna) con los ignorables YA filtrados. Hasta ahora solo se
// comprobaba de rebote (si estuviera mal, el parseo fallaría por otra razón) y
// para el caso puntual de INDENT/DEDENT.
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn contract_lexer_hands_the_parser_typed_tokens_with_real_positions() {
    let yal = read("workspace/compiscript.yal");
    let yalp = read("workspace/compiscript.yalp");
    let source = read("workspace/ejemplo.cps");

    let resp = api::build_pipeline_response_named(&yal, &yalp, &source, "lalr", "ejemplo.cps")
        .expect("el pipeline no debe fallar");

    let kinds: Vec<&str> = resp.token_map.iter().map(|t| t["kind"].as_str().unwrap()).collect();

    // La cabecera de `function suma(a: integer, b: integer): integer {`,
    // token por token. Fija a la vez el reconocimiento y el ORDEN.
    assert_eq!(
        &kinds[..14],
        &[
            "FUNCTION", "ID", "LPAREN", "ID", "COLON", "INT_T", "COMMA", "ID", "COLON", "INT_T",
            "RPAREN", "COLON", "INT_T", "LBRACE",
        ],
        "stream completo: {kinds:?}"
    );

    // Los kinds llegan en MAYÚSCULAS, que es la convención con la que el
    // .yalp declara sus %token: si el lexer entregara "Id" en vez de "ID", la
    // tabla del parser no encontraría la acción.
    assert!(
        kinds.iter().all(|k| k == &k.to_uppercase()),
        "todo kind debe estar normalizado a mayúsculas: {kinds:?}"
    );

    // Los lexemas y las posiciones son las del archivo real, no ceros.
    let first = &resp.token_map[0];
    assert_eq!(first["lexeme"], "function");
    assert_eq!(first["line"], 1);
    assert_eq!(first["col"], 1);

    let nombre = &resp.token_map[1];
    assert_eq!(nombre["kind"], "ID");
    assert_eq!(nombre["lexeme"], "suma", "el lexema se conserva, no solo el kind");
    assert_eq!(nombre["line"], 1);

    assert!(
        resp.token_map.iter().all(|t| t["line"].as_u64().unwrap() >= 1),
        "ninguna posición puede ser 0: la fase semántica ubica sus diagnósticos con esto"
    );
}

#[test]
fn contract_ignored_tokens_never_reach_the_parser() {
    // `ejemplo_closures.cps` no tiene comentarios; `clases_ok.cps` sí (`// ...`),
    // y COMMENT está declarado como token del lexer pero no debe llegar al
    // parser. Si el filtrado se rompiera, el parseo fallaría — pero por un
    // error sintáctico confuso, no por lo que realmente pasó.
    let yal = read("workspace/compiscript.yal");
    let yalp = read("workspace/compiscript.yalp");
    let source = read("workspace/clases_ok.cps");
    assert!(source.contains("//"), "la fuente debe tener comentarios para que el test valga");

    let resp = api::build_pipeline_response_named(&yal, &yalp, &source, "lalr", "clases_ok.cps")
        .expect("el pipeline no debe fallar");

    let kinds: Vec<&str> = resp.token_map.iter().map(|t| t["kind"].as_str().unwrap()).collect();
    assert!(
        !kinds.iter().any(|k| k.contains("COMMENT") || k.contains("WHITESPACE")),
        "comentarios y espacios no deben llegar al parser: {kinds:?}"
    );
    assert!(!kinds.is_empty());
}

// ════════════════════════════════════════════════════════════════════════════
// FRONTERA 2 · Sintáctico → Semántico
//
// Lo que la fase semántica recibe es un `ParseNode`. Los e2e solo comprobaban
// que existiera (`!parse_tree_dot.is_empty()`), nunca su FORMA.
// ════════════════════════════════════════════════════════════════════════════

const MINI: &str = "%token ID NUM PLUS SEMI\n\
                    %%\n\
                    programa : sent ;\n\
                    sent : expr SEMI ;\n\
                    expr : expr PLUS term | term ;\n\
                    term : ID | NUM ;\n";

fn mini_table() -> LRTable {
    let grammar = Grammar::parse_for_lr_from_str(MINI).expect("gramática válida");
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

/// Aplana el árbol a `padre>hijo` para poder compararlo completo sin depender
/// del formato del DOT.
fn shape(node: &ParseNode, out: &mut Vec<String>) {
    for c in &node.children {
        out.push(format!("{}>{}", node.symbol, c.symbol));
        shape(c, out);
    }
}

#[test]
fn contract_parser_hands_a_tree_shaped_like_the_grammar() {
    let table = mini_table();
    let tree = LRParser::new(&table)
        .parse_tree(toks(&["ID", "PLUS", "NUM", "SEMI"]))
        .expect("entrada válida");

    let mut edges = Vec::new();
    shape(&tree, &mut edges);

    // La forma COMPLETA, no solo la raíz. `a + b ;` tiene que derivar
    // exactamente así según las producciones de MINI.
    assert_eq!(
        edges,
        vec![
            "programa>sent",
            "sent>expr",
            "expr>expr",
            "expr>term",
            "term>ID",
            "expr>PLUS",
            "expr>term",
            "term>NUM",
            "sent>SEMI",
        ],
        "la forma del árbol debe seguir las producciones de la gramática"
    );
}

#[test]
fn contract_every_leaf_carries_a_real_position_for_the_semantic_phase() {
    let table = mini_table();
    let tree = LRParser::new(&table)
        .parse_tree(toks(&["ID", "PLUS", "NUM", "SEMI"]))
        .expect("entrada válida");

    // Invariante del que depende TODA la fase semántica: sin línea/columna en
    // las hojas, ningún diagnóstico se puede ubicar en el editor.
    fn check(n: &ParseNode) {
        if n.children.is_empty() {
            assert!(n.line > 0 && n.col > 0, "hoja sin posición: {} @{}:{}", n.symbol, n.line, n.col);
        }
        for c in &n.children {
            check(c);
        }
    }
    check(&tree);

    // Y un nodo interno hereda la posición de su primer hijo posicionado, para
    // que un diagnóstico sobre una producción entera sepa dónde empieza.
    assert_eq!((tree.line, tree.col), (1, 1), "la raíz hereda del primer token");

    assert!(to_dot(&tree).contains("programa"), "el DOT se genera desde el mismo árbol");
}

// ════════════════════════════════════════════════════════════════════════════
// FRONTERA 3 · Semántico → Fase 16 (código intermedio)
//
// Es la entrega peor cubierta y la que viene. Los e2e comprueban el TEXTO del
// volcado con `.contains(...)`; acá se comprueban los DATOS.
// ════════════════════════════════════════════════════════════════════════════

/// Compila, lexea, parsea y analiza de verdad, y devuelve la tabla de símbolos.
fn analyze_source(source_path: &str) -> SymbolTable {
    let yal = read("workspace/compiscript.yal");
    let yalp = read("workspace/compiscript.yalp");
    let source = read(source_path);

    let (_, _, lexer_table) = api::build_lexer_artifacts(&yal).expect("lexer válido");
    let grammar = Grammar::parse_for_lr_from_str(&yalp).expect("gramática válida");

    use lexer_generator::lexico::runtime::simulator::{LexResult, Simulator};
    let mut sim = Simulator::new(&lexer_table, &source);
    let mut tokens = Vec::new();
    loop {
        match sim.next_token() {
            LexResult::Token(t) => {
                let kind = t.kind.to_uppercase();
                if !grammar.ignores_kind(&kind) {
                    tokens.push(ParseToken { kind, lexeme: t.lexeme, line: t.line, col: t.col });
                }
            }
            LexResult::Error { lexeme, line, col } => {
                panic!("error léxico inesperado: '{lexeme}' en {line}:{col}")
            }
            LexResult::EOF => break,
        }
    }

    let first = calculate_first(&grammar);
    let lalr = merge_by_core(LR1Automaton::build(&grammar, &first));
    let table = LRTable::build_from_lalr(&lalr, &grammar);
    let tree = LRParser::new(&table).parse_tree(tokens).expect("fuente sintácticamente válida");

    let spec = SemanticSpec::from_grammar(&grammar).expect("compiscript.yalp trae %ident");
    analyze(&tree, &spec).table
}

#[test]
fn contract_function_signatures_survive_the_real_pipeline() {
    // `signature` lleva los tipos de los parámetros y el de retorno: es
    // EXACTAMENTE lo que la fase 16 necesita para emitir una llamada. Nada lo
    // verificaba de punta a punta — solo un unit test suelto sobre un árbol
    // armado a mano.
    let table = analyze_source("workspace/ejemplo.cps");

    let suma = table.lookup("suma").expect("`suma` se declaró");
    assert_eq!(suma.kind, SymbolKind::Function);

    let sig = suma.signature.as_ref().expect("una función debe tener firma");
    assert_eq!(
        sig.params,
        vec![Type::Int, Type::Int],
        "`function suma(a: integer, b: integer)` → dos parámetros enteros"
    );
    assert_eq!(sig.returns, Type::Int, "`: integer` es el tipo de retorno");
    assert_eq!(suma.ty, Some(Type::Int), "`ty` de una función es su tipo de retorno");
}

#[test]
fn contract_class_symbols_carry_parent_and_typed_members() {
    let table = analyze_source("workspace/clases_ok.cps");

    let figura = table.lookup("Figura").expect("`Figura` se declaró");
    assert_eq!(figura.kind, SymbolKind::Class);
    assert_eq!(figura.parent, None, "Figura no hereda de nadie");

    // `parent` es la cadena de herencia que `classes::resolve_member` recorre;
    // sin ella, un método heredado no se encuentra.
    let circulo = table.lookup("Circulo").expect("`Circulo` se declaró");
    assert_eq!(
        circulo.parent,
        Some("Figura".to_string()),
        "`class Circulo : Figura` debe dejar registrado el padre"
    );

    // Los miembros llegan con su tipo YA resuelto, no solo declarados.
    let members = figura.members.as_ref().expect("una clase cerrada tiene miembros");
    let area = members.iter().find(|m| m.name == "area").expect("Figura.area");
    assert_eq!(area.ty, Some(Type::Int), "el atributo llega tipado");

    let metodo = members
        .iter()
        .find(|m| m.name == "obtenerArea")
        .expect("Figura.obtenerArea");
    assert_eq!(metodo.kind, SymbolKind::Function);
    assert_eq!(
        metodo.signature.as_ref().map(|s| s.returns.clone()),
        Some(Type::Int),
        "un método también debe traer su firma"
    );
}

#[test]
fn contract_the_dump_agrees_with_the_underlying_data() {
    // Los e2e comprueban la tabla con `symbol_table.contains("x: Variable, Int")`
    // — o sea el TEXTO renderizado. Si el volcado y los datos se separaran, esos
    // tests seguirían pasando con datos malos. Acá se atan los dos.
    let table = analyze_source("workspace/ejemplo.cps");
    let dump = table.dump();

    let suma = table.lookup("suma").expect("`suma` está en la tabla");
    assert!(
        dump.contains("suma"),
        "lo que está en los datos tiene que aparecer en el volcado: {dump}"
    );
    assert!(
        dump.contains(&format!("@{}:{}", suma.line, suma.col)),
        "el volcado muestra la MISMA posición que guarda el símbolo ({}:{}): {dump}",
        suma.line,
        suma.col
    );

    // Y al revés: el volcado no inventa símbolos que no estén en la tabla.
    for linea in dump.lines() {
        let t = linea.trim();
        if let Some(nombre) = t.split(':').next().filter(|n| !n.is_empty() && !n.starts_with('[')) {
            if nombre.chars().all(|c| c.is_alphanumeric() || c == '_') && table.lookup(nombre).is_none() {
                // Los miembros anidados no son visibles con `lookup` desde el
                // scope global; solo se exige la correspondencia de los de
                // primer nivel, que sí lo son.
                assert!(
                    linea.starts_with("    ") || linea.starts_with('['),
                    "el volcado nombra '{nombre}', que no está en la tabla: {linea}"
                );
            }
        }
    }
}
