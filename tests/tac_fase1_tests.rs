//! Fase 1 del generador de código intermedio (ver `PLAN_TAC.md`):
//! expresiones, declaraciones, asignaciones y control de flujo, sobre el
//! pipeline real de Compiscript.
//!
//! Cada caso fija el TAC EXACTO de la función `main`: así una regresión en la
//! traducción (una etiqueta de más, un temporal que no se recicla, un salto
//! mal dirigido) rompe la prueba. Cuando exista el intérprete (Fase 3), estos
//! mismos programas pasan a la batería con su salida esperada.
//!
//! Las funciones y las clases son de la Fase 2: estos programas usan solo
//! código de nivel superior.
use lexer_generator::api;
use lexer_generator::intermedio::gen::generate;
use lexer_generator::intermedio::spec::IntermediateSpec;
use lexer_generator::intermedio::tac::TacProgram;
use lexer_generator::semantico::analyzer::analyze;
use lexer_generator::semantico::spec::SemanticSpec;
use lexer_generator::sintactico::automatas::lalr::merge_by_core;
use lexer_generator::sintactico::automatas::lr1::LR1Automaton;
use lexer_generator::sintactico::gramatica::first::calculate_first;
use lexer_generator::sintactico::gramatica::grammar::Grammar;
use lexer_generator::sintactico::runtime::parse_tree::ParseToken;
use lexer_generator::sintactico::runtime::parser_lr::LRParser;
use lexer_generator::sintactico::tablas::LRTable;
use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("no se pudo leer {path}: {e}"))
}

/// Lexea, parsea, analiza y genera `source` con Compiscript.
fn generar(source: &str) -> TacProgram {
    let (_, _, lexer_table) = api::build_lexer_artifacts(&read("workspace/compiscript.yal")).expect("lexer válido");
    let grammar = Grammar::parse_for_lr_from_str(&read("workspace/compiscript.yalp")).expect("gramática válida");

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

    let sspec = SemanticSpec::from_grammar(&grammar).expect("compiscript.yalp trae %ident");
    let analysis = analyze(&tree, &sspec);
    assert!(analysis.errors.is_empty(), "el programa no debe tener diagnósticos: {:?}", analysis.errors);
    let ispec = IntermediateSpec::from_grammar(&grammar);
    generate(&tree, &analysis, &sspec, &ispec).unwrap_or_else(|e| panic!("debe generar: {e:?}"))
}

/// El TAC de `main`, sin los espacios al final de cada línea.
fn main_tac(source: &str) -> String {
    let main = generar(source).main.expect("siempre hay main");
    main.to_string().lines().map(str::trim_end).collect::<Vec<_>>().join("\n")
}

#[test]
fn precedencia_y_reciclaje_de_temporales() {
    let tac = main_tac(
        "let a: integer = 1; let b: integer = 2; let c: integer = 3; let d: integer = 4;\n\
         let x: integer = a + b * c - d;\n\
         print(x);\n",
    );
    assert_eq!(
        tac,
        r#"func main nivel 0 marco 8
    a = 1
    b = 2
    c = 3
    d = 4
    t0 = b * c
    t0 = a + t0
    x = t0 - d
    print x
endfunc"#
    );
}

#[test]
fn el_reciclaje_deja_un_solo_temporal_vivo() {
    // `a + b * c - d` pide tres temporales, pero nunca hay más de uno vivo a
    // la vez: el marco reserva una sola ranura.
    let main = generar(
        "let a: integer = 1; let b: integer = 2; let c: integer = 3; let d: integer = 4;\n\
         let x: integer = a + b * c - d;\n",
    )
    .main
    .unwrap();
    assert_eq!(main.max_temps, 1);
    assert_eq!(main.temps_requested, 3);
}

#[test]
fn asociatividad_negacion_y_condiciones_literales() {
    // `a - b - c` es `(a - b) - c`. `while (true)` no evalúa nada.
    let tac = main_tac(
        "let a: integer = 9; let b: integer = 3; let c: integer = 1;\n\
         let x: integer = a - b - c;\n\
         let y: boolean = !(a < b);\n\
         let z: boolean = true;\n\
         if (z) { print(1); }\n\
         while (true) { break; }\n",
    );
    assert_eq!(
        tac,
        r#"func main nivel 0 marco 8
    a = 9
    b = 3
    c = 1
    t0 = a - b
    x = t0 - c
    t0 = a < b
    y = not t0
    z = true
    ifFalse z goto L0
    print 1
L0:
L1:
    goto L2
    goto L1
L2:
endfunc"#
    );
}

#[test]
fn division_modulo_concat_minus_y_cortocircuito_como_valor() {
    let tac = main_tac(
        "let a: integer = 10; let b: integer = 3;\n\
         let q: integer = a / b; let r: integer = a % b;\n\
         let s: string = \"ho\" + \"la\";\n\
         let n: integer = -a;\n\
         let ok: boolean = a < b && b != 0 || !(a == 1);\n\
         let nombre: string;\n",
    );
    assert_eq!(
        tac,
        r#"func main nivel 0 marco 8
    a = 10
    b = 3
    check_zero b
    q = a / b
    check_zero b
    r = a % b
    s = "ho" concat "la"
    n = minus a
    if a >= b goto L3
    if b != 0 goto L2
L3:
    if a == 1 goto L0
L2:
    t0 = true
    goto L1
L0:
    t0 = false
L1:
    ok = t0
    nombre = null
endfunc"#
    );
}

#[test]
fn if_else_if_else_y_if_sin_else() {
    let tac = main_tac(
        "let a: integer = 1;\n\
         if (a < 3 && a != 0) { print(1); } else if (a == 5) { print(2); } else { print(3); }\n\
         if (a > 0) { print(4); }\n",
    );
    assert_eq!(
        tac,
        r#"func main nivel 0 marco 0
    a = 1
    if a >= 3 goto L1
    if a == 0 goto L1
    print 1
    goto L0
L1:
    if a != 5 goto L3
    print 2
    goto L2
L3:
    print 3
L2:
L0:
    if a <= 0 goto L4
    print 4
L4:
endfunc"#
    );
}

#[test]
fn while_do_while_y_for_con_break_y_continue() {
    // `continue` va a la condición en `while`, y al paso en `for`.
    let tac = main_tac(
        "let i: integer = 0;\n\
         while (i < 10) { if (i == 5) { break; } i = i + 1; if (i == 2) { continue; } print(i); }\n\
         do { i = i - 1; } while (i > 0);\n\
         for (let j: integer = 0; j < 3; j = j + 1) { if (j == 1) { continue; } print(j); }\n\
         for (; i < 2; ) { i = i + 1; }\n",
    );
    assert_eq!(
        tac,
        r#"func main nivel 0 marco 8
    i = 0
L0:
    if i >= 10 goto L1
    if i != 5 goto L2
    goto L1
L2:
    i = i + 1
    if i != 2 goto L3
    goto L0
L3:
    print i
    goto L0
L1:
L4:
    i = i - 1
L5:
    if i > 0 goto L4
L6:
    j = 0
L7:
    if j >= 3 goto L9
    if j != 1 goto L10
    goto L8
L10:
    print j
L8:
    j = j + 1
    goto L7
L9:
L11:
    if i >= 2 goto L13
    i = i + 1
L12:
    goto L11
L13:
endfunc"#
    );
}

#[test]
fn foreach_recorre_la_lista_por_indice() {
    // La lista: `[longitud][e0][e1]…`, elementos de 4 bytes desde el byte 4.
    let tac = main_tac(
        "let notas: integer[];\n\
         foreach (n in notas) { if (n == 0) { continue; } print(n); }\n",
    );
    assert_eq!(
        tac,
        r#"func main nivel 0 marco 32
    notas = null
    t0 = notas
    t1 = t0[0]
    t2 = 0
L0:
    if t2 >= t1 goto L2
    t3 = t2 * 4
    t3 = t3 + 4
    n = t0[t3]
    if n != 0 goto L3
    goto L1
L3:
    print n
L1:
    t2 = t2 + 1
    goto L0
L2:
endfunc"#
    );
}

#[test]
fn switch_cae_entre_casos_y_continue_sigue_al_bucle_de_afuera() {
    let tac = main_tac(
        "let x: integer = 2;\n\
         switch (x) { case 1: print(1); case 2: print(2); break; default: print(0); }\n\
         while (true) { switch (x) { case 2: continue; } break; }\n",
    );
    assert_eq!(
        tac,
        r#"func main nivel 0 marco 8
    x = 2
    t0 = x
    if t0 == 1 goto L1
    if t0 == 2 goto L2
    goto L3
L1:
    print 1
L2:
    print 2
    goto L0
L3:
    print 0
L0:
L4:
    t0 = x
    if t0 == 2 goto L7
    goto L6
L7:
    goto L4
L6:
    goto L5
    goto L4
L5:
endfunc"#
    );
}

#[test]
fn try_catch_y_un_break_que_sale_del_try_cierra_su_manejador() {
    let tac = main_tac(
        "let x: integer = 0;\n\
         while (x < 3) { try { x = 10 / x; break; } catch (e) { print(e); } x = x + 1; }\n",
    );
    assert_eq!(
        tac,
        r#"func main nivel 0 marco 8
    x = 0
L0:
    if x >= 3 goto L1
    try_begin L2
    check_zero x
    x = 10 / x
    try_end
    goto L1
    try_end
    goto L3
L2:
    e = exception
    print e
L3:
    x = x + 1
    goto L0
L1:
endfunc"#
    );
}

#[test]
fn una_variable_local_con_el_nombre_de_una_global_lleva_sufijo() {
    let tac = main_tac("let x: integer = 1;\n{ let x: integer = 2; print(x); }\nprint(x);\n");
    assert!(tac.contains("    x = 1\n    x.1 = 2\n    print x.1\n    print x\n"), "{tac}");
}
