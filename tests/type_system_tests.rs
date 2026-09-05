use lexer_generator::semantico::symbols::{SemanticError, SymbolKind, SymbolTable};
use lexer_generator::semantico::types::{
    resolve_arithmetic, resolve_assignment, ArithmeticOperator, ArithmeticResolution, Coercion,
    Type, TypeError, TYPE_COMPATIBILITY,
};

const OPERATORS: [ArithmeticOperator; 4] = [
    ArithmeticOperator::Add,
    ArithmeticOperator::Subtract,
    ArithmeticOperator::Multiply,
    ArithmeticOperator::Divide,
];

#[test]
fn arithmetic_table_resolves_every_integer_float_combination() {
    let cases = [
        (
            Type::Int,
            Type::Int,
            ArithmeticResolution {
                result: Type::Int,
                left_coercion: Coercion::Exact,
                right_coercion: Coercion::Exact,
            },
        ),
        (
            Type::Int,
            Type::Float,
            ArithmeticResolution {
                result: Type::Float,
                left_coercion: Coercion::IntToFloat,
                right_coercion: Coercion::Exact,
            },
        ),
        (
            Type::Float,
            Type::Int,
            ArithmeticResolution {
                result: Type::Float,
                left_coercion: Coercion::Exact,
                right_coercion: Coercion::IntToFloat,
            },
        ),
        (
            Type::Float,
            Type::Float,
            ArithmeticResolution {
                result: Type::Float,
                left_coercion: Coercion::Exact,
                right_coercion: Coercion::Exact,
            },
        ),
    ];

    for operator in OPERATORS {
        for (left, right, expected) in &cases {
            assert_eq!(
                resolve_arithmetic(operator, left, right),
                Ok(expected.clone()),
                "falló {left} {operator} {right}"
            );
        }
    }
}

#[test]
fn arithmetic_table_rejects_non_numeric_operands_for_every_operator() {
    for operator in OPERATORS {
        assert_eq!(
            TYPE_COMPATIBILITY.arithmetic(operator, &Type::Bool, &Type::Int),
            Err(TypeError::InvalidArithmetic {
                operator,
                left: Type::Bool,
                right: Type::Int,
            })
        );
        assert!(resolve_arithmetic(operator, &Type::Str, &Type::Float).is_err());
    }
}

#[test]
fn assignment_table_allows_exact_types_and_safe_numeric_widening() {
    for ty in [Type::Int, Type::Float, Type::Bool, Type::Str, Type::Void] {
        assert_eq!(resolve_assignment(&ty, &ty), Ok(Coercion::Exact));
    }

    assert_eq!(
        resolve_assignment(&Type::Float, &Type::Int),
        Ok(Coercion::IntToFloat)
    );
    assert_eq!(
        resolve_assignment(
            &Type::Array(Box::new(Type::Int)),
            &Type::Array(Box::new(Type::Int))
        ),
        Ok(Coercion::Exact)
    );
    assert_eq!(
        resolve_assignment(
            &Type::Named("Punto".to_string()),
            &Type::Named("Punto".to_string())
        ),
        Ok(Coercion::Exact)
    );
    assert_eq!(
        resolve_assignment(&Type::Unknown, &Type::Unknown),
        Ok(Coercion::Exact)
    );
}

#[test]
fn assignment_table_rejects_narrowing_and_unrelated_types() {
    assert_eq!(
        resolve_assignment(&Type::Int, &Type::Float),
        Err(TypeError::IncompatibleAssignment {
            expected: Type::Int,
            found: Type::Float,
        })
    );
    assert!(resolve_assignment(&Type::Bool, &Type::Int).is_err());
    assert!(resolve_assignment(
        &Type::Named("Punto".to_string()),
        &Type::Named("Vector".to_string())
    )
    .is_err());
}

#[test]
fn unknown_is_neutral_on_both_sides_of_an_assignment() {
    // `Unknown` significa "esta fase todavía no supo tipar la expresión", no
    // un tipo incompatible. Usarlo para rechazar convierte cada hueco de
    // tipado en un diagnóstico falso: un `%type_of` que apunta a un terminal
    // sin `%type_token`, o un `const K = f();` cuyo inicializador no se sabe
    // tipar, hacían que toda declaración/asignación/argumento de ese símbolo
    // reportara S006/S014. Antes solo se aceptaba `Unknown` contra `Unknown`.
    for known in [
        Type::Int,
        Type::Float,
        Type::Bool,
        Type::Str,
        Type::Void,
        Type::Named("Punto".to_string()),
    ] {
        assert_eq!(
            resolve_assignment(&Type::Unknown, &known),
            Ok(Coercion::Exact),
            "destino sin tipar, valor {known}"
        );
        assert_eq!(
            resolve_assignment(&known, &Type::Unknown),
            Ok(Coercion::Exact),
            "destino {known}, valor sin tipar"
        );
    }

    assert_eq!(
        resolve_assignment(&Type::Unknown, &Type::Unknown),
        Ok(Coercion::Exact)
    );
}

#[test]
fn an_unknown_operand_makes_arithmetic_unknown_instead_of_invalid() {
    for operator in OPERATORS {
        for (left, right) in [
            (Type::Unknown, Type::Int),
            (Type::Float, Type::Unknown),
            (Type::Unknown, Type::Unknown),
            // Ni siquiera contra un tipo que jamás sería válido: no sabemos
            // el otro operando, así que no hay nada que afirmar.
            (Type::Unknown, Type::Str),
        ] {
            let resolved = resolve_arithmetic(operator, &left, &right)
                .unwrap_or_else(|e| panic!("{left} {operator} {right} no debe fallar: {e}"));
            assert_eq!(
                resolved.result,
                Type::Unknown,
                "{left} {operator} {right} debe propagar Unknown"
            );
        }
    }

    // Permisivo con lo desconocido, NO con lo conocido: dos tipos que sí
    // sabemos tipar y son incompatibles siguen siendo un error.
    assert!(resolve_arithmetic(ArithmeticOperator::Add, &Type::Bool, &Type::Str).is_err());
}

#[test]
fn typed_declaration_and_assignment_use_the_declared_type() {
    let mut table = SymbolTable::new();
    table
        .declare_typed(
            "promedio",
            SymbolKind::Variable,
            Type::Float,
            true,
            false,
            None,
            1,
            1,
        )
        .unwrap();

    assert!(!table.lookup("promedio").unwrap().initialized);
    assert_eq!(
        table.assign("promedio", Some(&Type::Int), 2, 1),
        Ok(Some(Coercion::IntToFloat))
    );
    let symbol = table.lookup("promedio").unwrap();
    assert_eq!(symbol.ty, Some(Type::Float));
    assert!(symbol.initialized);
}

#[test]
fn incompatible_assignment_is_rejected_without_marking_initialized() {
    let mut table = SymbolTable::new();
    table
        .declare_typed(
            "cantidad",
            SymbolKind::Variable,
            Type::Int,
            true,
            false,
            None,
            1,
            1,
        )
        .unwrap();

    assert_eq!(
        table.assign("cantidad", Some(&Type::Float), 3, 7),
        Err(SemanticError::AssignmentTypeMismatch {
            name: "cantidad".to_string(),
            expected: Type::Int,
            found: Type::Float,
            line: 3,
            col: 7,
        })
    );
    assert!(!table.lookup("cantidad").unwrap().initialized);
}

#[test]
fn const_requires_a_compatible_initializer_and_cannot_be_reassigned() {
    let mut table = SymbolTable::new();

    assert_eq!(
        table.declare_typed(
            "SIN_VALOR",
            SymbolKind::Variable,
            Type::Int,
            false,
            false,
            None,
            4,
            3,
        ),
        Err(SemanticError::ConstRequiresInitializer {
            name: "SIN_VALOR".to_string(),
            line: 4,
            col: 3,
        })
    );
    assert!(table.lookup("SIN_VALOR").is_none());

    assert!(matches!(
        table.declare_typed(
            "ENTERO_INVALIDO",
            SymbolKind::Variable,
            Type::Int,
            false,
            true,
            Some(Type::Float),
            5,
            3,
        ),
        Err(SemanticError::AssignmentTypeMismatch { .. })
    ));
    assert!(table.lookup("ENTERO_INVALIDO").is_none());

    table
        .declare_typed(
            "PI_APROX",
            SymbolKind::Variable,
            Type::Float,
            false,
            true,
            Some(Type::Int),
            6,
            3,
        )
        .unwrap();
    let constant = table.lookup("PI_APROX").unwrap();
    assert!(!constant.mutable);
    assert!(constant.initialized);
    assert_eq!(constant.ty, Some(Type::Float));

    assert_eq!(
        table.assign("PI_APROX", Some(&Type::Float), 7, 3),
        Err(SemanticError::AssignmentToConst {
            name: "PI_APROX".to_string(),
            line: 7,
            col: 3,
        })
    );
}

/// Los tipos compuestos son compatibles solo consigo mismos, y esa regla vive
/// en UNA linea de `types::CompatibilityTable::assignment`. Un compuesto que
/// se agregue al `enum Type` sin nombrarse ahi cae a la tabla de primitivos,
/// no encuentra fila, y termina siendo incompatible HASTA CONSIGO MISMO —
/// este test es la red que atrapa ese olvido.
#[test]
fn every_compound_type_is_compatible_with_itself_and_only_with_itself() {
    let mapa = |k: Type, v: Type| Type::Map(Box::new(k), Box::new(v));
    let conjunto = |t: Type| Type::Set(Box::new(t));
    let arreglo = |t: Type| Type::Array(Box::new(t));

    let iguales = [
        (arreglo(Type::Int), arreglo(Type::Int)),
        (conjunto(Type::Str), conjunto(Type::Str)),
        (mapa(Type::Str, Type::Int), mapa(Type::Str, Type::Int)),
        (
            Type::Tuple(vec![Type::Str, Type::Int]),
            Type::Tuple(vec![Type::Str, Type::Int]),
        ),
    ];
    for (expected, found) in &iguales {
        assert!(
            resolve_assignment(expected, found).is_ok(),
            "{expected} debe ser compatible consigo mismo"
        );
    }

    let distintos = [
        // Mismo constructor, parametro distinto.
        (mapa(Type::Str, Type::Int), mapa(Type::Str, Type::Str)),
        (conjunto(Type::Int), conjunto(Type::Str)),
        // Misma "forma" pero constructor distinto: un conjunto de enteros no
        // es un arreglo de enteros.
        (conjunto(Type::Int), arreglo(Type::Int)),
        // Tuplas de distinta aridad y de distinto orden.
        (
            Type::Tuple(vec![Type::Str, Type::Int]),
            Type::Tuple(vec![Type::Str]),
        ),
        (
            Type::Tuple(vec![Type::Str, Type::Int]),
            Type::Tuple(vec![Type::Int, Type::Str]),
        ),
    ];
    for (expected, found) in &distintos {
        assert!(
            resolve_assignment(expected, found).is_err(),
            "{expected} NO debe aceptar {found}"
        );
    }
}

#[test]
fn el_mas_concatena_dos_textos() {
    assert_eq!(
        resolve_arithmetic(ArithmeticOperator::Add, &Type::Str, &Type::Str),
        Ok(ArithmeticResolution {
            result: Type::Str,
            left_coercion: Coercion::Exact,
            right_coercion: Coercion::Exact,
        }),
        "`\"a\" + \"b\"` debe dar string: lo usa la especificación de Compiscript"
    );
}

#[test]
fn solo_el_mas_concatena_los_otros_operadores_siguen_rechazando_texto() {
    // Este test es el que justifica que la concatenación NO sea una fila de
    // `ARITHMETIC_RULES`: esa matriz la comparten los cuatro operadores y la
    // búsqueda ignora cuál es, así que una fila `Str, Str` allí habría hecho
    // legales también estas tres.
    for operator in [
        ArithmeticOperator::Subtract,
        ArithmeticOperator::Multiply,
        ArithmeticOperator::Divide,
    ] {
        assert_eq!(
            resolve_arithmetic(operator, &Type::Str, &Type::Str),
            Err(TypeError::InvalidArithmetic {
                operator,
                left: Type::Str,
                right: Type::Str,
            }),
            "`\"a\" {operator} \"b\"` no tiene sentido"
        );
    }
}

#[test]
fn concatenar_texto_con_un_numero_sigue_siendo_invalido() {
    // Compiscript se describe como un subconjunto de TypeScript, donde
    // `"x" + 1` sí es válido. Acá NO: mezclar texto y número es el caso que
    // verifican las baterías de las cuatro gramáticas (S015), y ampliarlo
    // dejaría de detectar errores de tipo reales.
    for (left, right) in [(Type::Str, Type::Int), (Type::Int, Type::Str)] {
        assert!(
            resolve_arithmetic(ArithmeticOperator::Add, &left, &right).is_err(),
            "`{left} + {right}` debe seguir siendo inválido"
        );
    }
}
