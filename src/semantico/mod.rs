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
// Lo que sigue AÚN NO IMPLEMENTADO: el chequeo de tipos, y conectar esto a
// la API HTTP (ver ORGANIZACION.md § "Fases futuras" para el roadmap
// completo).
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
// errores. Ninguno de los dos se invoca hoy desde la API HTTP — antes de
// conectar esta fase al pipeline hay que resolver la duplicación de drivers
// shift-reduce documentada en ORGANIZACION.md (5 variantes hoy).

pub mod analyzer;
pub mod scopes;
pub mod spec;
pub mod symbols;
