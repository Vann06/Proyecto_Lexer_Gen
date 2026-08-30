use lexer_generator::api;
use lexer_generator::semantico::analyzer::analyze;
use lexer_generator::semantico::spec::SemanticSpec;
use lexer_generator::sintactico::gramatica::grammar::Grammar;
use lexer_generator::sintactico::runtime::parse_tree::ParseNode;
const FLOW_OK: &str = r#"
let activo: boolean = true;
while (activo) {
    if (activo) { continue; } else { break; }
}
function revisar(valor: boolean): integer {
    while (valor) { break; }
    return 1;
}
"#;

const FLOW_ERRORS: &str = r#"
if (1) { print(1); }
while ("texto") { print(1); }
break;
continue;
while (true) {
    function interna(): integer {
        break;
        continue;
        return 1;
    }
    break;
    continue;
}
"#;

#[test]
fn compiscript_valid_flow_converges_with_the_real_pipeline() {
    let yal = include_str!("../workspace/compiscript.yal");
    let yalp = include_str!("../workspace/compiscript.yalp");

    let response = api::build_pipeline_response_named(yal, yalp, FLOW_OK, "lalr", "flow_ok.cps")
        .expect("el pipeline no debe fallar");

    assert!(
        response.accepted,
        "flow_ok.cps debe ser sintácticamente válido: {:?} / {:#?}",
        response.error, response.problems
    );
    assert!(
        response.problems.is_empty(),
        "caso válido: {:#?}",
        response.problems
    );
    assert!(!response.parse_tree_dot.is_empty());
    assert!(!response.symbol_table.is_empty());
}

#[test]
fn compiscript_invalid_flow_reports_each_rule_with_real_locations() {
    let yal = include_str!("../workspace/compiscript.yal");
    let yalp = include_str!("../workspace/compiscript.yalp");

    let response =
        api::build_pipeline_response_named(yal, yalp, FLOW_ERRORS, "lalr", "flow_errores.cps")
            .expect("el pipeline no debe fallar");

    assert!(
        response.accepted,
        "los errores son semánticos, no sintácticos: {:?} / {:#?}",
        response.error, response.problems
    );

    let codes: Vec<&str> = response
        .problems
        .iter()
        .filter_map(|problem| problem["code"].as_str())
        .collect();
    let count = |code: &str| codes.iter().filter(|found| **found == code).count();

    assert_eq!(count("S025"), 2, "if(integer) y while(string): {codes:?}");
    assert_eq!(
        count("S026"),
        2,
        "break global y dentro de función anidada: {codes:?}"
    );
    assert_eq!(
        count("S027"),
        2,
        "continue global y dentro de función anidada: {codes:?}"
    );
    assert_eq!(
        codes.len(),
        6,
        "no debe haber diagnósticos derivados: {:#?}",
        response.problems
    );

    for problem in &response.problems {
        assert!(problem["loc"]
            .as_str()
            .unwrap()
            .starts_with("flow_errores.cps:"));
        assert!(problem["line"].as_u64().unwrap() > 0);
        assert!(problem["col"].as_u64().unwrap() > 0);
    }
}

#[test]
fn flow_rules_do_not_depend_on_compiscript_names() {
    let grammar = Grammar::parse_for_lr_from_str(
        "%token ID SI REPETIR SALIR SIGUIENTE VERDAD NUM\n\
         %ident ID\n\
         %type_token VERDAD bool\n\
         %type_token NUM integer\n\
         %condition decision condicion\n\
         %condition ciclo condicion\n\
         %loop ciclo\n\
         %break salir\n\
         %continue siguiente\n\
         %%\n\
         programa : decision ciclo salir siguiente ;\n\
         decision : SI condicion ;\n\
         ciclo : REPETIR condicion bloque ;\n\
         bloque : salir ;\n\
         salir : SALIR ;\n\
         siguiente : SIGUIENTE ;\n\
         condicion : VERDAD | NUM ;\n",
    )
    .expect("la gramática alternativa debe ser válida");
    let spec = SemanticSpec::from_grammar(&grammar).expect("trae %ident");

    let tree = internal(
        "programa",
        vec![
            internal(
                "decision",
                vec![
                    leaf("SI", "si", 1, 1),
                    internal("condicion", vec![leaf("NUM", "1", 1, 4)]),
                ],
            ),
            internal(
                "ciclo",
                vec![
                    leaf("REPETIR", "repetir", 2, 1),
                    internal("condicion", vec![leaf("VERDAD", "verdad", 2, 9)]),
                    internal(
                        "bloque",
                        vec![internal("salir", vec![leaf("SALIR", "salir", 3, 3)])],
                    ),
                ],
            ),
            internal("salir", vec![leaf("SALIR", "salir", 5, 1)]),
            internal("siguiente", vec![leaf("SIGUIENTE", "siguiente", 6, 1)]),
        ],
    );

    let result = analyze(&tree, &spec);
    let codes: Vec<&str> = result
        .errors
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert_eq!(codes, vec!["S025", "S026", "S027"]);
}

/// `%condition <produccion> <indice>` es la forma que documenta
/// `src/semantico/flow/README.md` para las construcciones donde la condicion
/// NO es simplemente "el primer hijo con tal simbolo": un `for` la lleva
/// despues del inicializador, y un `do-while` despues del cuerpo. Compiscript
/// no tiene ninguna de las dos (solo `if`/`while`, donde alcanza con buscar
/// por simbolo), asi que esa rama de `child_locator_from_directive` no la
/// ejercitaba nada — justo la que hace falta para cubrir `for`/`foreach`/
/// `do-while` cuando la gramatica los tenga.
#[test]
fn a_positional_condition_index_picks_the_right_child_for_for_and_do_while() {
    let grammar = Grammar::parse_for_lr_from_str(
        "%token ID PARA HACER SALIR SEGUIR VERDAD NUM\n\
         %ident ID\n\
         %type_token VERDAD bool\n\
         %type_token NUM integer\n\
         %condition para 2\n\
         %loop para\n\
         %condition repetir 2\n\
         %loop repetir\n\
         %break salir\n\
         %continue seguir\n\
         %%\n\
         programa : para repetir ;\n\
         para : PARA valor valor cuerpo ;\n\
         repetir : HACER cuerpo valor ;\n\
         cuerpo : salir | seguir ;\n\
         salir : SALIR ;\n\
         seguir : SEGUIR ;\n\
         valor : VERDAD | NUM ;\n",
    )
    .expect("la gramática posicional debe ser válida");
    let spec = SemanticSpec::from_grammar(&grammar).expect("trae %ident");

    let valor = |token: &str, lexeme: &str, line: usize, col: usize| {
        internal("valor", vec![leaf(token, lexeme, line, col)])
    };

    // `para PARA <init:NUM> <cond:VERDAD> <cuerpo>` — el inicializador es un
    // entero y la condicion un booleano. Con el indice 2 se valida la
    // CONDICION; si la regla cayera en "el primer hijo `valor`" señalaría el
    // inicializador y reportaría un S025 falso.
    let para = internal(
        "para",
        vec![
            leaf("PARA", "para", 1, 1),
            valor("NUM", "0", 1, 6),
            valor("VERDAD", "verdad", 1, 9),
            internal("cuerpo", vec![internal("salir", vec![leaf("SALIR", "salir", 2, 3)])]),
        ],
    );

    // `repetir HACER <cuerpo> <cond:NUM>` — do-while con la condicion DESPUES
    // del cuerpo, y de tipo entero: ese sí es un S025 real.
    let repetir = internal(
        "repetir",
        vec![
            leaf("HACER", "hacer", 4, 1),
            internal("cuerpo", vec![internal("seguir", vec![leaf("SEGUIR", "seguir", 5, 3)])]),
            valor("NUM", "1", 6, 9),
        ],
    );

    let result = analyze(&internal("programa", vec![para, repetir]), &spec);
    let codes: Vec<&str> = result.errors.iter().map(|d| d.code.as_str()).collect();

    assert_eq!(
        codes,
        vec!["S025"],
        "solo la condicion entera del do-while; el `break`/`continue` estan dentro de sus bucles \
         y el inicializador entero del `for` no es una condicion: {codes:?}"
    );
    let diagnostic = result.errors.iter().next().expect("hay un S025");
    assert_eq!(
        (diagnostic.line, diagnostic.col),
        (6, 9),
        "debe señalar la condicion del do-while, no el cuerpo ni el inicializador"
    );
}

fn leaf(symbol: &str, lexeme: &str, line: usize, col: usize) -> ParseNode {
    ParseNode {
        symbol: symbol.to_string(),
        lexeme: Some(lexeme.to_string()),
        children: vec![],
        line,
        col,
    }
}

fn internal(symbol: &str, children: Vec<ParseNode>) -> ParseNode {
    ParseNode::internal(symbol.to_string(), children)
}
