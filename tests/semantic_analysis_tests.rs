// Prueba de que src/semantico/analyzer::analyze es genuinamente agnóstico a
// la gramática: dos gramáticas REALES y distintas (compiladas, lexeadas y
// parseadas de verdad — no árboles armados a mano), cada una con su propio
// SemanticSpec chico, corridas por el MISMO `analyze() `.
use lexer_generator::api;
use lexer_generator::lexico::runtime::simulator::{LexResult, Simulator};
use lexer_generator::semantico::analyzer::analyze;
use lexer_generator::semantico::scopes::ScopeKind;
use lexer_generator::semantico::errors::ErrorKind;
use lexer_generator::semantico::spec::{DeclarationRule, ScopeRule, SemanticSpec};
use lexer_generator::semantico::symbols::SymbolKind;
use lexer_generator::sintactico::automatas::lalr::merge_by_core;
use lexer_generator::sintactico::automatas::lr1::LR1Automaton;
use lexer_generator::sintactico::gramatica::first::calculate_first;
use lexer_generator::sintactico::gramatica::grammar::Grammar;
use lexer_generator::sintactico::runtime::parse_tree::{ParseNode, ParseToken};
use lexer_generator::sintactico::runtime::parser_lr::LRParser;
use lexer_generator::sintactico::tablas::LRTable;
use std::fs;

/// Compila+lexea+parsea `source` de verdad contra el `.yal`/`.yalp` dados y
/// devuelve el árbol de derivación real — mismo camino que usa la CLI
/// (`src/bin/test_pipeline.rs`), no un atajo de test.
fn real_parse_tree(yal: &str, yalp: &str, source: &str) -> ParseNode {
    let (_, _, table) = api::build_lexer_artifacts(yal).expect("lexer válido");
    let grammar = Grammar::parse_for_lr_from_str(yalp).expect("gramática válida");

    let mut sim = Simulator::new(&table, source);
    let mut tokens = Vec::new();
    loop {
        match sim.next_token() {
            LexResult::Token(t) => {
                let kind = t.kind.to_uppercase();
                let lower = kind.to_lowercase();
                let ignored = lower.contains("whitespace")
                    || lower.contains("comment")
                    || lower == "ignored"
                    || grammar.ignores.contains(&kind);
                if !ignored {
                    tokens.push(ParseToken { kind, lexeme: t.lexeme, line: t.line, col: t.col });
                }
            }
            LexResult::Error { lexeme, line, col } => {
                panic!("error léxico inesperado: '{lexeme}' en {line}:{col}")
            }
            LexResult::EOF => break,
        }
    }

    let first_sets = calculate_first(&grammar);
    let lr1 = LR1Automaton::build(&grammar, &first_sets);
    let lalr = merge_by_core(lr1);
    let lr_table = LRTable::build_from_lalr(&lalr, &grammar);
    LRParser::new(&lr_table)
        .parse_tree(tokens)
        .expect("la fuente de prueba debe ser sintácticamente válida para esta gramática")
}

#[test]
fn miniprog_symbol_table_from_real_grammar() {
    let yal = fs::read_to_string("examples/lexer/miniprog.yal").expect("existe miniprog.yal");
    let yalp = fs::read_to_string("examples/grammar/miniprog.yalp").expect("existe miniprog.yalp");

    let source = r#"
        fun suma(int a, int b) : int {
          int resultado = a + b;
          print(faltante);
          return resultado;
        }
    "#;

    let spec = SemanticSpec {
        identifier_token: "ID".to_string(),
        declarations: vec![
            DeclarationRule {
                production: "func_decl".to_string(),
                kind: SymbolKind::Function,
                name_child: None,
                implicit: false,
            },
            DeclarationRule {
                production: "param".to_string(),
                kind: SymbolKind::Parameter,
                name_child: None,
                implicit: false,
            },
            DeclarationRule {
                production: "var_decl".to_string(),
                kind: SymbolKind::Variable,
                name_child: None,
                implicit: false,
            },
        ],
        scopes: vec![
            ScopeRule { production: "func_decl".to_string(), kind: ScopeKind::Function, with_label: true },
            ScopeRule { production: "bloque".to_string(), kind: ScopeKind::Block, with_label: false },
        ],
    };

    let tree = real_parse_tree(&yal, &yalp, source);
    let result = analyze(&tree, &spec);

    // Un solo error real: "faltante" nunca se declaró.
    assert_eq!(result.errors.len(), 1, "errores: {:?}", result.errors);
    let diag = result.errors.iter().next().unwrap();
    assert_eq!(diag.kind, ErrorKind::Ambito);
    assert!(diag.message.contains("faltante"));

    // La función quedó declarada en Global, con los scopes ya cerrados.
    assert_eq!(result.table.depth(), 1);
    let suma = result.table.lookup("suma").expect("suma se declaró");
    assert_eq!(suma.kind, SymbolKind::Function);

    // "a"/"b"/"resultado" eran locales a la función — no visibles desde afuera...
    assert_eq!(result.table.lookup("a"), None);
    assert_eq!(result.table.lookup("resultado"), None);
    // ...y los parámetros (declarados DIRECTO en el scope Function que abrió
    // func_decl) siguen consultables colgados de "suma" sin recorrer el
    // árbol de nuevo. "resultado" NO aparece acá — vive un nivel más adentro
    // (dentro del `bloque` del cuerpo, un scope Block anidado que func_decl
    // no declaró) — ver el comentario sobre este límite en analyzer::walk.
    let member_names: Vec<&str> = suma
        .members
        .as_ref()
        .expect("func_decl abrió un scope, debe haber quedado members")
        .iter()
        .map(|m| m.name.as_str())
        .collect();
    assert!(member_names.contains(&"a"));
    assert!(member_names.contains(&"b"));
    assert!(!member_names.contains(&"resultado"), "resultado vive en el bloque anidado, no directo en Function");
}

#[test]
fn classes_functions_symbol_table_from_real_grammar() {
    let yal = fs::read_to_string("examples/cases/05_classes_functions/lexer.yal")
        .expect("existe 05_classes_functions/lexer.yal");
    let yalp = fs::read_to_string("examples/cases/05_classes_functions/grammar.yapar")
        .expect("existe 05_classes_functions/grammar.yapar");

    // "z" nunca se declara ni se asigna — debe fallar por no-declarada.
    let source = "def foo ( x , y ) { return x + z ; }";

    let spec = SemanticSpec {
        identifier_token: "ID".to_string(),
        declarations: vec![
            DeclarationRule {
                production: "func_decl".to_string(),
                kind: SymbolKind::Function,
                name_child: None,
                implicit: false,
            },
            DeclarationRule {
                production: "class_decl".to_string(),
                kind: SymbolKind::Class,
                name_child: None,
                implicit: false,
            },
            DeclarationRule {
                production: "param_list".to_string(),
                kind: SymbolKind::Parameter,
                name_child: None,
                implicit: false,
            },
            // Sin var_decl separado en esta gramática — declarar es
            // implícito en la primera asignación (ID ASSIGN expr).
            DeclarationRule {
                production: "stmt".to_string(),
                kind: SymbolKind::Variable,
                name_child: None,
                implicit: true,
            },
        ],
        scopes: vec![
            ScopeRule { production: "func_decl".to_string(), kind: ScopeKind::Function, with_label: true },
            ScopeRule { production: "class_decl".to_string(), kind: ScopeKind::Class, with_label: true },
        ],
    };

    let tree = real_parse_tree(&yal, &yalp, source);
    let result = analyze(&tree, &spec);

    assert_eq!(result.errors.len(), 1, "errores: {:?}", result.errors);
    let diag = result.errors.iter().next().unwrap();
    assert_eq!(diag.kind, ErrorKind::Ambito);
    assert!(diag.message.contains('z'));

    assert_eq!(result.table.depth(), 1);
    assert_eq!(result.table.lookup("foo").expect("foo se declaró").kind, SymbolKind::Function);
    // "x"/"y" eran parámetros locales a foo.
    assert_eq!(result.table.lookup("x"), None);
}

/// Misma gramática de clases+funciones, pero probando específicamente el
/// caso que motivó `DeclarationRule::implicit`: reasignar un parámetro
/// dentro del cuerpo de la función no debe reportarse como redeclaración.
#[test]
fn classes_functions_reassigning_a_parameter_is_not_a_redeclaration() {
    let yal = fs::read_to_string("examples/cases/05_classes_functions/lexer.yal")
        .expect("existe 05_classes_functions/lexer.yal");
    let yalp = fs::read_to_string("examples/cases/05_classes_functions/grammar.yapar")
        .expect("existe 05_classes_functions/grammar.yapar");

    // Tal como examples/cases/05_classes_functions/input.txt: "x" es
    // parámetro Y se reasigna dentro del cuerpo.
    let source = "def foo ( x , y ) { x = x + y ; return x ; }";

    let spec = SemanticSpec {
        identifier_token: "ID".to_string(),
        declarations: vec![
            DeclarationRule {
                production: "func_decl".to_string(),
                kind: SymbolKind::Function,
                name_child: None,
                implicit: false,
            },
            DeclarationRule {
                production: "param_list".to_string(),
                kind: SymbolKind::Parameter,
                name_child: None,
                implicit: false,
            },
            DeclarationRule {
                production: "stmt".to_string(),
                kind: SymbolKind::Variable,
                name_child: None,
                implicit: true,
            },
        ],
        scopes: vec![ScopeRule {
            production: "func_decl".to_string(),
            kind: ScopeKind::Function,
            with_label: true,
        }],
    };

    let tree = real_parse_tree(&yal, &yalp, source);
    let result = analyze(&tree, &spec);

    assert!(result.errors.is_empty(), "no debería haber errores: {:?}", result.errors);
}

/// `members` no se queda en un solo nivel: una clase con un método adentro
/// debe poder consultarse en cascada (MyClass -> bar -> a) sin volver a
/// tocar el árbol — es justo el caso que el libro del dragón pide poder
/// resolver para acceso a miembros / cálculo de activation records.
#[test]
fn classes_functions_class_members_are_queryable_after_the_walk() {
    let yal = fs::read_to_string("examples/cases/05_classes_functions/lexer.yal")
        .expect("existe 05_classes_functions/lexer.yal");
    let yalp = fs::read_to_string("examples/cases/05_classes_functions/grammar.yapar")
        .expect("existe 05_classes_functions/grammar.yapar");

    let source = "class MyClass { def bar ( a ) { return a ; } }";

    let spec = SemanticSpec {
        identifier_token: "ID".to_string(),
        declarations: vec![
            DeclarationRule {
                production: "class_decl".to_string(),
                kind: SymbolKind::Class,
                name_child: None,
                implicit: false,
            },
            DeclarationRule {
                production: "func_decl".to_string(),
                kind: SymbolKind::Function,
                name_child: None,
                implicit: false,
            },
            DeclarationRule {
                production: "param_list".to_string(),
                kind: SymbolKind::Parameter,
                name_child: None,
                implicit: false,
            },
        ],
        scopes: vec![
            ScopeRule { production: "class_decl".to_string(), kind: ScopeKind::Class, with_label: true },
            ScopeRule { production: "func_decl".to_string(), kind: ScopeKind::Function, with_label: true },
        ],
    };

    let tree = real_parse_tree(&yal, &yalp, source);
    let result = analyze(&tree, &spec);
    assert!(result.errors.is_empty(), "no debería haber errores: {:?}", result.errors);

    let my_class = result.table.lookup("MyClass").expect("MyClass se declaró");
    assert_eq!(my_class.kind, SymbolKind::Class);
    let class_members = my_class.members.as_ref().expect("class_decl abrió un scope");
    let bar = class_members.iter().find(|m| m.name == "bar").expect("bar es método de MyClass");
    assert_eq!(bar.kind, SymbolKind::Function);

    // Un nivel más abajo: los parámetros de "bar" cuelgan de "bar", no de "MyClass".
    let bar_members = bar.members.as_ref().expect("func_decl abrió un scope");
    assert!(bar_members.iter().any(|m| m.name == "a" && m.kind == SymbolKind::Parameter));
}
