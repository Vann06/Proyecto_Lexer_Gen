//! El contrato de traspaso a la fase de código intermedio (ARQUITECTURA.md §8).
//!
//! Además del árbol y el tipo de cada expresión (`type_annotations_tests.rs`),
//! la traducción del capítulo 6 del libro del dragón necesita tres cosas que
//! el análisis antes calculaba y tiraba:
//!
//! 1. a qué declaración apunta cada identificador (`bindings`),
//! 2. qué operando necesita una ampliación `int -> float` (`widen`, §6.5.2),
//! 3. dónde vive cada variable y cuánto pesa cada marco/objeto (cap. 7).
//!
//! Todo por el pipeline real, sobre Compiscript. La ampliación (2) se prueba
//! en los tests unitarios de `analyzer`: Compiscript no tiene `float`.
use lexer_generator::api;
use lexer_generator::intermedio::spec::IntermediateSpec;
use lexer_generator::sintactico::automatas::lalr::merge_by_core;
use lexer_generator::sintactico::automatas::lr1::LR1Automaton;
use lexer_generator::sintactico::gramatica::first::calculate_first;
use lexer_generator::sintactico::gramatica::flow::{FlowKind, FlowRole};
use lexer_generator::sintactico::gramatica::grammar::Grammar;
use lexer_generator::sintactico::runtime::parse_tree::{ParseNode, ParseToken};
use lexer_generator::sintactico::runtime::parser_lr::LRParser;
use lexer_generator::sintactico::tablas::LRTable;
use serde_json::Value;
use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("no se pudo leer {path}: {e}"))
}

fn compiscript(source: &str) -> api::ParseResponse {
    let yal = read("workspace/compiscript.yal");
    let yalp = read("workspace/compiscript.yalp");
    let r = api::build_pipeline_response_named(&yal, &yalp, source, "lalr", "traspaso.cps")
        .expect("el pipeline no debe fallar internamente");
    assert!(r.accepted, "debe parsear: {:?}", r.error);
    assert!(r.problems.is_empty(), "no debe haber diagnósticos: {:#?}", r.problems);
    r
}

/// `(scope_id, decl_index)` de cada aparición de `name`, en orden de lectura.
fn refs_de<'a>(bindings: &'a [Value], name: &str) -> Vec<(u64, u64, u64)> {
    bindings
        .iter()
        .filter(|b| b["lexeme"] == name)
        .map(|b| (b["line"].as_u64().unwrap(), b["scope_id"].as_u64().unwrap(), b["decl_index"].as_u64().unwrap()))
        .collect()
}

#[test]
fn cada_uso_apunta_a_su_declaracion_respetando_el_ocultamiento() {
    let r = compiscript(
        "let x: integer = 1;\n\
         function f(): integer {\n\
           let x: integer = 2;\n\
           return x;\n\
         }\n\
         x = x + 1;\n",
    );
    let xs = refs_de(&r.bindings, "x");
    // Línea 1 (declaración global), 3 (declaración local), 4 (uso local), 6
    // (destino y uso globales).
    let global: Vec<_> = xs.iter().filter(|(l, _, _)| *l == 1 || *l == 6).collect();
    let local: Vec<_> = xs.iter().filter(|(l, _, _)| *l == 3 || *l == 4).collect();
    assert_eq!(global.len(), 3, "declaración + destino + uso globales: {xs:?}");
    assert_eq!(local.len(), 2, "declaración + uso locales: {xs:?}");
    assert!(global.iter().all(|(_, s, _)| *s == 0), "la x de afuera vive en el Global: {xs:?}");
    let local_scope = local[0].1;
    assert_ne!(local_scope, 0);
    assert!(local.iter().all(|(_, s, _)| *s == local_scope), "la x de adentro es una sola: {xs:?}");
}

#[test]
fn los_locales_de_una_funcion_reciben_offset_y_la_funcion_su_marco() {
    let r = compiscript(
        "function f(a: integer, b: integer): integer {\n\
           let c: integer = a + b;\n\
           return c;\n\
         }\n",
    );
    // Los parámetros viven en el ámbito de la función; `c`, en el bloque del
    // cuerpo, que no tiene marco propio: comparte el de `f`.
    let scope = r
        .scopes
        .iter()
        .find(|s| s["kind"] == "Function")
        .expect("f abre un ámbito de función");
    let offset = |name: &str| {
        r.scopes
            .iter()
            .flat_map(|s| s["symbols"].as_array().unwrap().iter())
            .find(|s| s["name"] == name)
            .and_then(|s| s["offset"].as_i64())
            .unwrap_or_else(|| panic!("{name} debe tener offset: {:#?}", r.scopes))
    };
    // MIPS por defecto: $fp+0 enlace de control, +4 valor de retorno, +8
    // enlace de acceso; parámetros desde $fp+12 hacia arriba, locales desde
    // $fp-8 hacia abajo (ver `storage::TargetLayout`).
    assert_eq!(offset("a"), 12);
    assert_eq!(offset("b"), 16);
    assert_eq!(offset("c"), -12);

    let id = scope["id"].as_u64().unwrap();
    assert!(
        r.layout.contains(&format!("marco #{id} f (nivel 1): 12 bytes")),
        "área fija de 8 + un local de 4: {}",
        r.layout
    );
}

#[test]
fn una_clase_hija_empieza_donde_termina_su_padre() {
    let r = compiscript(
        "class Figura {\n\
           var area: integer = 0;\n\
         }\n\
         class Circulo : Figura {\n\
           var radio: integer = 0;\n\
         }\n",
    );
    assert!(r.layout.contains("clase Figura: 4 bytes"), "{}", r.layout);
    assert!(r.layout.contains("clase Circulo: 8 bytes"), "{}", r.layout);
}

#[test]
fn en_la_rubrica_solo_el_area_estatica_queda_incompleta() {
    // `let d = null;` no tiene un tipo del que sacar un ancho: el área
    // estática queda incompleta —TAC no debe inventarle un tamaño—, pero
    // todas las funciones y clases se ubican.
    let r = compiscript(&read("workspace/rubrica.cps"));
    assert!(!r.bindings.is_empty());
    let incompletos: Vec<&str> = r.layout.lines().filter(|l| l.contains("[incompleto]")).collect();
    assert_eq!(incompletos.len(), 1, "{}", r.layout);
    assert!(incompletos[0].starts_with("marco #0 estático:"), "{}", r.layout);
    assert!(r.layout.contains("clase Animal:") && r.layout.contains("clase Perro:"), "{}", r.layout);
}

/// El símbolo `name` en alguna foto de ámbito que cumpla `pred`.
fn simbolo_en<'a>(scopes: &'a [Value], name: &str, pred: impl Fn(&Value) -> bool) -> &'a Value {
    scopes
        .iter()
        .filter(|s| pred(s))
        .flat_map(|s| s["symbols"].as_array().unwrap().iter())
        .find(|s| s["name"] == name)
        .unwrap_or_else(|| panic!("no está `{name}`: {scopes:#?}"))
}

#[test]
fn this_es_el_primer_parametro_oculto_de_un_metodo() {
    // clases_ok.cps:4 — `function constructor(areaInicial: integer)` de Figura.
    let r = compiscript(&read("workspace/clases_ok.cps"));
    let es_ctor_de_figura = |s: &Value| {
        s["kind"] == "Function"
            && s["symbols"].as_array().unwrap().iter().any(|x| x["name"] == "areaInicial")
    };
    let this = simbolo_en(&r.scopes, "this", es_ctor_de_figura);
    assert_eq!(this["kind"], "Parameter");
    assert_eq!(this["offset"], 12, "primera ranura de parámetro");
    let area = simbolo_en(&r.scopes, "areaInicial", es_ctor_de_figura);
    assert_eq!(area["offset"], 16, "los parámetros del usuario van después de `this`");

    // `this.area = areaInicial;` (línea 5): el `this` se enlaza, en su marco.
    let usos: Vec<&Value> = r.bindings.iter().filter(|b| b["lexeme"] == "this" && b["line"] == 5).collect();
    assert_eq!(usos.len(), 1, "{:#?}", r.bindings);
    assert_eq!(usos[0]["access"], "local");
}

#[test]
fn la_variable_del_catch_es_un_string_con_lugar_en_el_marco() {
    // rubrica.cps:82-83 — `catch (err) { print("Error atrapado: " + err); }`
    let r = compiscript(&read("workspace/rubrica.cps"));
    let err = simbolo_en(&r.scopes, "err", |s| s["kind"] == "Block");
    assert_eq!(err["ty"], "string");
    assert!(err["offset"].is_i64(), "err debe tener offset: {err:#}");
    assert_eq!(accesos(&r.bindings, "err", 83), vec![("static".to_string(), None)], "el try está fuera de toda función");
}

// ═════════════════════════ %flow sobre el árbol real ═════════════════════════

/// Lexea y parsea `source` con Compiscript y devuelve el árbol real junto con
/// la gramática (mismo camino que `contract_tests::analyze_source`).
fn arbol_real(source: &str) -> (Grammar, ParseNode) {
    let yal = read("workspace/compiscript.yal");
    let yalp = read("workspace/compiscript.yalp");
    let (_, _, lexer_table) = api::build_lexer_artifacts(&yal).expect("lexer válido");
    let grammar = Grammar::parse_for_lr_from_str(&yalp).expect("gramática válida");

    use lexer_generator::lexico::runtime::simulator::{LexResult, Simulator};
    let mut sim = Simulator::new(&lexer_table, source);
    let mut tokens = Vec::new();
    loop {
        match sim.next_token() {
            LexResult::Token(t) => {
                let kind = t.kind.to_uppercase();
                if !grammar.ignores_kind(&kind) {
                    tokens.push(ParseToken { kind, lexeme: t.lexeme, line: t.line, col: t.col });
                }
            }
            LexResult::Error { lexeme, line, col } => panic!("error léxico: '{lexeme}' en {line}:{col}"),
            LexResult::EOF => break,
        }
    }
    let first = calculate_first(&grammar);
    let table = LRTable::build_from_lalr(&merge_by_core(LR1Automaton::build(&grammar, &first)), &grammar);
    let tree = LRParser::new(&table).parse_tree(tokens).expect("fuente válida");
    (grammar, tree)
}

/// Todos los nodos con símbolo `symbol`, en preorden.
fn nodos<'a>(node: &'a ParseNode, symbol: &str, out: &mut Vec<&'a ParseNode>) {
    if node.symbol == symbol {
        out.push(node);
    }
    for c in &node.children {
        nodos(c, symbol, out);
    }
}

fn todos<'a>(tree: &'a ParseNode, symbol: &str) -> Vec<&'a ParseNode> {
    let mut out = Vec::new();
    nodos(tree, symbol, &mut out);
    out
}

/// El texto que cubre un subárbol, para comparar sin depender de su forma.
fn texto(node: &ParseNode) -> String {
    match &node.lexeme {
        Some(l) => l.clone(),
        None => node.children.iter().map(texto).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" "),
    }
}

#[test]
fn flow_encuentra_cada_parte_del_if_con_y_sin_else() {
    let (grammar, tree) = arbol_real(
        "let n: integer = 0;
         if (n < 1) { n = 1; }
         if (n < 2) { n = 2; } else if (n < 3) { n = 3; } else { n = 4; }
",
    );
    let spec = IntermediateSpec::from_grammar(&grammar);
    let ifs = todos(&tree, "if_stmt");
    assert_eq!(ifs.len(), 3, "dos if de primer nivel + el `else if` anidado");

    let corto = spec.flow_of(ifs[0]).expect("if_stmt tiene %flow");
    assert_eq!(corto.kind(), FlowKind::If);
    assert_eq!(texto(corto.child(ifs[0], FlowRole::Cond).unwrap()), "n < 1");
    assert_eq!(texto(corto.child(ifs[0], FlowRole::Then).unwrap()), "{ n = 1 ; }");
    assert!(corto.child(ifs[0], FlowRole::Else).is_none(), "if sin else");

    let largo = spec.flow_of(ifs[1]).unwrap();
    let rama_falsa = largo.child(ifs[1], FlowRole::Else).expect("tiene else");
    assert_eq!(rama_falsa.symbol, "if_stmt", "`else if` encadena otro if_stmt");
    assert!(std::ptr::eq(rama_falsa, ifs[2]));
    assert_eq!(texto(largo.child(ifs[2], FlowRole::Else).unwrap()), "{ n = 4 ; }");
}

#[test]
fn flow_encuentra_las_partes_de_los_bucles_y_el_switch() {
    let (grammar, tree) = arbol_real(
        "let n: integer = 0;
         for (; n < 10; ) { n = n + 1; }
         for (let i: integer = 0; i < 3; i = i + 1) { n = i; }
         while (n > 5) { n = n - 1; }
         do { n = n - 1; } while (n > 0);
         switch (n) { case 1: n = 2; default: n = 3; }
",
    );
    let spec = IntermediateSpec::from_grammar(&grammar);

    let fors = todos(&tree, "for_stmt");
    let f = spec.flow_of(fors[0]).unwrap();
    assert_eq!(f.kind(), FlowKind::For);
    assert!(f.child(fors[0], FlowRole::Init).is_none(), "for_init vacío = no hay inicialización");
    assert!(f.child(fors[0], FlowRole::Update).is_none());
    assert_eq!(texto(f.child(fors[0], FlowRole::Cond).unwrap()), "n < 10");
    assert_eq!(texto(f.child(fors[1], FlowRole::Init).unwrap()), "let i : integer = 0");
    assert_eq!(texto(f.child(fors[1], FlowRole::Update).unwrap()), "i = i + 1");
    assert_eq!(texto(f.child(fors[1], FlowRole::Body).unwrap()), "{ n = i ; }");

    let w = todos(&tree, "while_stmt")[0];
    let shape = spec.flow_of(w).unwrap();
    assert_eq!(texto(shape.child(w, FlowRole::Cond).unwrap()), "n > 5");

    let d = todos(&tree, "do_while_stmt")[0];
    let shape = spec.flow_of(d).unwrap();
    assert_eq!(shape.kind(), FlowKind::DoWhile);
    assert_eq!(texto(shape.child(d, FlowRole::Body).unwrap()), "{ n = n - 1 ; }");
    assert_eq!(texto(shape.child(d, FlowRole::Cond).unwrap()), "n > 0");

    let sw = todos(&tree, "switch_stmt")[0];
    let shape = spec.flow_of(sw).unwrap();
    assert_eq!(texto(shape.child(sw, FlowRole::Disc).unwrap()), "n");
    let caso = todos(&tree, "switch_case")[0];
    let shape = spec.flow_of(caso).unwrap();
    assert_eq!(texto(shape.child(caso, FlowRole::Value).unwrap()), "1");
    assert_eq!(texto(shape.child(caso, FlowRole::Body).unwrap()), "n = 2 ;");
    let def = todos(&tree, "default_case")[0];
    assert_eq!(texto(spec.flow_of(def).unwrap().child(def, FlowRole::Body).unwrap()), "n = 3 ;");
}

// ═══════════════════════════ Enlace de acceso (§7.3) ═════════════════════════

/// `access` (y `hops`) de cada aparición de `name` en la línea `line`.
fn accesos(bindings: &[Value], name: &str, line: u64) -> Vec<(String, Option<u64>)> {
    bindings
        .iter()
        .filter(|b| b["lexeme"] == name && b["line"] == line)
        .map(|b| (b["access"].as_str().unwrap().to_string(), b["hops"].as_u64()))
        .collect()
}

#[test]
fn una_funcion_anidada_llega_a_la_variable_de_afuera_por_un_enlace() {
    // contador (nivel 1) declara `total`; incrementar (nivel 2) la usa.
    let r = compiscript(&read("workspace/ejemplo_closures.cps"));
    let nonlocal = ("nonlocal".to_string(), Some(1));
    let local = ("local".to_string(), None);

    // Línea 2: `let total` en su propia función.
    assert_eq!(accesos(&r.bindings, "total", 2), vec![local.clone()]);
    // Línea 5: `total = total + paso;` dentro de incrementar — destino y uso
    // de `total` a un salto; `paso` es propio.
    assert_eq!(accesos(&r.bindings, "total", 5), vec![nonlocal.clone(), nonlocal.clone()]);
    assert_eq!(accesos(&r.bindings, "paso", 5), vec![local.clone()]);
    // Línea 11: `return total;` de vuelta en contador.
    assert_eq!(accesos(&r.bindings, "total", 11), vec![local]);

    let resultado = r.bindings.iter().find(|b| b["lexeme"] == "resultado").expect("resultado se enlaza");
    assert_eq!(resultado["access"], "static");

    assert!(r.layout.contains("contador (nivel 1)"), "{}", r.layout);
    assert!(r.layout.contains("incrementar (nivel 2)"), "{}", r.layout);
}

#[test]
fn un_campo_usado_a_secas_dentro_de_un_metodo_se_alcanza_por_this() {
    // `return radio;` dentro de Circulo.obtenerRadio (clases_ok.cps:26).
    let r = compiscript(&read("workspace/clases_ok.cps"));
    assert_eq!(accesos(&r.bindings, "radio", 26), vec![("field".to_string(), None)]);
    // Un método es nivel 1, igual que una función global.
    assert!(r.layout.contains("obtenerRadio (nivel 1)"), "{}", r.layout);
}

#[test]
fn flow_encuentra_el_cuerpo_y_el_manejador_de_un_try() {
    let (grammar, tree) = arbol_real("try { print(1); } catch (e) { print(e); }
");
    let spec = IntermediateSpec::from_grammar(&grammar);

    let t = todos(&tree, "try_stmt")[0];
    let shape = spec.flow_of(t).expect("try_stmt tiene %flow");
    assert_eq!(shape.kind(), FlowKind::Try);
    assert_eq!(texto(shape.child(t, FlowRole::Body).unwrap()), "{ print ( 1 ) ; }");
    let manejador = shape.child(t, FlowRole::Handler).unwrap();
    assert_eq!(manejador.symbol, "catch_clause");

    let c = spec.flow_of(manejador).expect("catch_clause tiene %flow");
    assert_eq!(c.kind(), FlowKind::Catch);
    assert_eq!(texto(c.child(manejador, FlowRole::Var).unwrap()), "e");
    assert_eq!(texto(c.child(manejador, FlowRole::Body).unwrap()), "{ print ( e ) ; }");
}
