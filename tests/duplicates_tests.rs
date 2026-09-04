use lexer_generator::api;
use lexer_generator::semantico::analyzer::analyze;
use lexer_generator::semantico::spec::SemanticSpec;
use lexer_generator::semantico::symbols::{SemanticError, SymbolKind, SymbolTable};
use lexer_generator::semantico::types::Type;
use lexer_generator::sintactico::gramatica::grammar::Grammar;
use lexer_generator::sintactico::runtime::parse_tree::ParseNode;

const YAL: &str = include_str!("../workspace/compiscript.yal");
const YALP: &str = include_str!("../workspace/compiscript.yalp");
const CASES: &str = include_str!("../workspace/duplicates_casos.txt");

#[test]
fn ide_suite_checks_each_rule_through_the_real_pipeline() {
    // Cada línea es un programa independiente para el IDE. Los bloques también
    // permiten comprobar todas las líneas en una sola petición al pipeline.
    let expected = [
        None,
        Some("S001"),
        None,
        Some("W001"),
        Some("W001"),
        Some("S001"),
        None,
        Some("W001"),
        None,
        Some("W001"),
        Some("W001"),
        None,
        Some("W001"),
        Some("S001"),
        None,
    ];
    assert_eq!(CASES.lines().count(), expected.len());
    let grammar = format!("%warn_unused\n{YALP}");
    for mode in ["lalr", "slr"] {
        let response =
            api::build_pipeline_response_named(YAL, &grammar, CASES, mode, "duplicates_casos.txt")
                .expect("la suite debe poder analizarse desde el mismo pipeline del IDE");
        assert!(response.accepted, "{mode}: {:?}", response.error);
        assert!(!response.parse_tree_dot.is_empty());
        for (index, expected_code) in expected.iter().enumerate() {
            let line = index + 1;
            let problems: Vec<_> = response
                .problems
                .iter()
                .filter(|problem| problem["line"].as_u64() == Some(line as u64))
                .collect();
            let codes: Vec<_> = problems
                .iter()
                .map(|p| p["code"].as_str().unwrap())
                .collect();
            assert_eq!(
                codes,
                expected_code.iter().copied().collect::<Vec<_>>(),
                "{mode}, caso {line}: {problems:?}"
            );
            for problem in problems {
                let code = problem["code"].as_str().unwrap();
                assert_eq!(
                    problem["level"],
                    if code == "W001" { "warn" } else { "err" }
                );
                let col = problem["col"].as_u64().unwrap();
                assert!(col > 0);
                assert_eq!(problem["loc"], format!("duplicates_casos.txt:{line}:{col}"));
            }
        }
        assert_eq!(response.problems.len(), expected.iter().flatten().count());
        assert!(
            !response.closures.is_empty(),
            "la captura debe seguir funcionando"
        );
        assert!(
            !response.scopes.is_empty(),
            "las fotos de ámbitos se conservan"
        );
    }
}

#[test]
fn duplicate_with_invalid_initializer_preserves_the_first_symbol() {
    let mut table = SymbolTable::new();
    table
        .declare_typed(
            "dato",
            SymbolKind::Variable,
            Type::Int,
            true,
            true,
            Some(Type::Int),
            1,
            5,
        )
        .unwrap();
    let original = table.lookup("dato").unwrap().clone();
    let error = table
        .declare_typed(
            "dato",
            SymbolKind::Variable,
            Type::Str,
            true,
            true,
            Some(Type::Int),
            2,
            7,
        )
        .unwrap_err();
    assert_eq!(
        error,
        SemanticError::Redeclared {
            name: "dato".into(),
            line: 2,
            col: 7,
            first_line: 1,
            first_col: 5,
        }
    );
    assert_eq!(table.lookup("dato"), Some(&original));
}

fn alternative_spec(warnings: bool) -> SemanticSpec {
    let flag = if warnings { "%warn_unused\n" } else { "" };
    let grammar = Grammar::parse_for_lr_from_str(&format!(
        "{flag}%token NOMBRE NUM\n\
         %ident NOMBRE\n\
         %declare guardar variable\n\
         %init_of guardar valor\n\
         %type_token NUM integer\n\
         %%\n\
         programa : guardar lectura ;\n\
         guardar : NOMBRE valor ;\n\
         valor : NUM ;\n\
         lectura : NOMBRE | ;\n"
    ))
    .unwrap();
    SemanticSpec::from_grammar(&grammar).unwrap()
}

fn leaf(symbol: &str, lexeme: &str, line: usize, col: usize) -> ParseNode {
    ParseNode {
        symbol: symbol.into(),
        lexeme: Some(lexeme.into()),
        children: vec![],
        line,
        col,
    }
}

fn alternative_tree(read: bool) -> ParseNode {
    let declaration = ParseNode::internal(
        "guardar".into(),
        vec![
            leaf("NOMBRE", "dato", 1, 1),
            ParseNode::internal("valor".into(), vec![leaf("NUM", "1", 1, 6)]),
        ],
    );
    let uses = if read {
        vec![leaf("NOMBRE", "dato", 2, 1)]
    } else {
        vec![]
    };
    ParseNode::internal(
        "programa".into(),
        vec![declaration, ParseNode::internal("lectura".into(), uses)],
    )
}

#[test]
fn unused_global_is_opt_in_and_independent_of_language_names() {
    let tree = alternative_tree(false);
    let old_behavior = analyze(&tree, &alternative_spec(false));
    assert!(old_behavior.errors.is_empty());
    let result = analyze(&tree, &alternative_spec(true));
    let problems = result.errors.to_problems("otro_lenguaje.txt");
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0]["code"], "W001");
    assert_eq!(problems[0]["loc"], "otro_lenguaje.txt:1:1");
    assert!(!result.table.lookup("dato").unwrap().used);
}

#[test]
fn a_real_read_marks_the_resolved_global_symbol() {
    let result = analyze(&alternative_tree(true), &alternative_spec(true));
    assert!(result.errors.is_empty());
    assert!(result.table.lookup("dato").unwrap().used);
}

#[test]
fn type_lookups_and_writes_are_not_reads() {
    let mut table = SymbolTable::new();
    table.declare("dato", SymbolKind::Variable, 1, 1).unwrap();
    assert!(table.lookup("dato").is_some());
    table.assign("dato", Some(&Type::Int), 2, 1).unwrap();
    assert!(!table.lookup("dato").unwrap().used);
}
