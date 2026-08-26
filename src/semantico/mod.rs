// Fase 15 (libro del dragón): análisis semántico — tabla de símbolos, alcance
// y chequeo de tipos.
//
// `scopes`/`symbols` implementan la tabla de símbolos con entornos anidados
// (global/función/clase/bloque), declare/lookup con shadowing correcto y
// volcado del estado. `spec`/`analyzer` implementan el walker: `analyzer`
// recorre un `ParseNode` real y llama a esa tabla según lo que diga un
// `spec::SemanticSpec` — un mapeo declarativo chico (qué producción declara
// qué, cuál abre un scope) que es lo único específico de una gramática
// concreta. El walker en sí (`analyzer::walk`) no menciona ningún nombre de
// producción — ver sus propios doc-comments para el diseño completo,
// incluido el porqué de `DeclarationRule::implicit`.
//
// El submódulo `types` concentra el enum de tipos, la tabla de compatibilidad
// y las coerciones para aritmética/asignaciones. `symbols` usa esas reglas al
// declarar y asignar símbolos tipados, incluida la inicialización obligatoria
// de constantes.
//
// `classes` resuelve miembros con `.` subiendo la cadena de herencia, `this`,
// y la invocación del constructor. `functions` es dueño de la comprobación de
// argumentos contra una firma (`check_arguments` — la ÚNICA implementación de
// esa regla; `classes` la usa para constructores y llamadas) y de la
// validación de `return` contra el tipo declarado (`FunctionContext`).
// `closures` acumula qué función anidada captura qué variables libres de su
// entorno de definición.
// `flow` valida condiciones booleanas y el contexto de los saltos de bucle.
//
// Los tipos registro (structs) definidos por el usuario reusan casi toda la
// maquinaria de clases: `SymbolKind::Struct`/`ScopeKind::Struct` los
// distinguen en la tabla, sus campos quedan como `Symbol.members` por el
// mismo mecanismo genérico que cierra cualquier scope, su tipo es un
// `Type::Named` con compatibilidad nominal, y `classes::resolve_member`
// resuelve `p.campo`. Lo propio de un struct es el literal con campos
// nombrados (`Punto { x: 1, y: 2 }`), que `classes::validate_struct_literal`
// comprueba contra los campos declarados.
//
// Esta fase YA está conectada a la API HTTP: `api::pipeline` la corre sobre
// el árbol real y expone `problems`, `symbol_table` y `closures` (ver
// `api::build_pipeline_response_named`).
//
// Restricción de diseño, no negociable: el generador es agnóstico a la
// gramática (cualquier .yal/.yalp/.txt que se reciba, no un lenguaje fijo
// hardcodeado aquí). Todo este módulo la respeta — nada acá sabe de ninguna
// gramática concreta; la especificidad vive exclusivamente en el
// `SemanticSpec` que arma quien reciba la gramática real (ver
// `tests/semantic_analysis_tests.rs` para dos ejemplos completos con
// gramáticas distintas de verdad).
//
// Punto de entrada del walker: el `ParseNode` que ya construyen
// `sintactico::runtime::parser_lr::LRParser::parse_tree` /
// `parse_recovering_with_pos` y `sintactico::runtime::ll1::LL1Parser::
// parse_tree` — ambos ya anotan cada hoja con `line`/`col` (ver
// `sintactico::runtime::parse_tree::ParseNode`), que es justo lo que
// `symbols::SymbolTable::declare`/`lookup_or_err` esperan para ubicar sus
// errores.

pub mod analyzer;
pub mod classes;
pub mod closures;
pub mod errors;
pub mod flow;
pub mod functions;
pub mod scopes;
pub mod spec;
pub mod symbols;
pub mod types;
pub mod visitor;
