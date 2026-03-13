# Guía de Implementación: Generador de Analizadores Léxicos (YALex → Lexer)
### Basada en *Compilers: Principles, Techniques, and Tools* (2nd Ed.) — Aho, Lam, Sethi, Ullman

---

## Flujo Completo del Pipeline

```
archivo .yal
   │
   ▼
src/spec/parser.rs        ← Fase 1: leer y tokenizar el .yal
   │
   ▼
src/spec/ast.rs           ← Fase 2: estructuras SpecIR, Definition, Rule
   │
   ▼
src/spec/expand.rs        ← Fase 3: expandir {MACROS} en regex completas
   │
   ▼
src/regex/parser.rs       ← Fase 4: regex → AST
src/regex/ast.rs          ← Fase 5: nodos del árbol regex
   │
   ▼
src/graph/dot.rs          ← Fase 6: exportar AST/AFN/AFD a .dot
   │
   ▼
src/automata/nfa.rs       ← Fase 7: Thompson NFA por regla
   │
   ▼
src/automata/subset.rs    ← Fase 8: construcción de subconjuntos NFA→DFA
src/automata/dfa.rs       ← Fase 9: estructura del DFA
   │
   ▼
src/automata/minimize.rs  ← Fase 10: minimización del DFA
   │
   ▼
src/table/transition_table.rs  ← Fase 11: delta[state][symbol]
   │
   ▼
src/runtime/simulator.rs  ← Fase 12: maximal munch + tokens
   │
   ▼
src/codegen/rust_codegen.rs ← Fase 13: generar lexer.rs
   │
   ▼
generated/lexer.rs
```

---

## Fase 0 — `src/main.rs`

### Propósito
Punto de entrada. Coordina el pipeline completo.

### Qué hace
- Lee argumentos de línea de comandos (ruta del `.yal`)
- Llama las fases en orden
- Maneja errores globales
- Decide dónde guardar resultados

### Esqueleto

```rust
use std::env;
use std::fs;

mod spec;
mod regex;
mod automata;
mod table;
mod runtime;
mod codegen;
mod graph;
mod error;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: yalex <file.yal>");
        std::process::exit(1);
    }
    let source = fs::read_to_string(&args[1]).expect("No se pudo leer el archivo");

    // 1. Parsear especificación
    let spec_ir = spec::parser::parse(&source).unwrap_or_else(|e| {
        eprintln!("Error en spec: {}", e); std::process::exit(1)
    });

    // 2. Expandir macros
    let expanded = spec::expand::expand(&spec_ir).unwrap_or_else(|e| {
        eprintln!("Error expandiendo: {}", e); std::process::exit(1)
    });

    // 3. Construir NFA global
    let nfa = automata::nfa::build_global_nfa(&expanded);

    // 4. NFA → DFA → minimizar
    let dfa = automata::subset::build_dfa(&nfa);
    let dfa_min = automata::minimize::minimize(&dfa);

    // 5. Tabla de transición
    let table = table::transition_table::build(&dfa_min);

    // 6. Graficar (opcional)
    graph::dot::write_dfa_file("output/dfa.dot", &dfa_min);

    // 7. Generar lexer
    codegen::rust_codegen::emit_file("generated/lexer.rs", &table, &expanded);

    println!("Lexer generado en generated/lexer.rs");
}
```

### Entrada / Salida

| | |
|---|---|
| **Entrada** | ruta del archivo `.yal` como argumento CLI |
| **Salida** | coordinación del pipeline; archivo `generated/lexer.rs` |

---

## Fase 1 — `src/spec/parser.rs`

### Propósito
Leer el archivo `.yal` y separar sus partes en datos estructurados.

### Estructura de un archivo `.yal`

```
{ header_code }

let DIGIT = ['0'-'9']
let LETTER = ['a'-'z' 'A'-'Z']
let ID = LETTER (LETTER | DIGIT)*

rule tokens =
  | DIGIT+         { return NUM }
  | ID             { return IDENT }
  | " "            { (* skip *) }
  | _              { raise LexError }
```

### Qué parsear

| Sección | Descripción |
|---|---|
| `{ ... }` al inicio | Código de encabezado (header) |
| `let NAME = regex` | Definiciones / macros |
| `rule NAME =` | Inicio de la sección de reglas |
| `\| regex { acción }` | Par regex-acción con prioridad por orden |
| `{ ... }` al final | Trailer / código auxiliar |

### Implementación en Rust

```rust
// src/spec/parser.rs
use crate::spec::ast::{SpecIR, Definition, Rule};
use crate::error::LexError;

pub fn parse(source: &str) -> Result<SpecIR, LexError> {
    let mut ir = SpecIR::default();
    let mut chars = source.chars().peekable();

    // 1. Header opcional: { ... }
    ir.header = parse_block(&mut chars)?;

    // 2. Definiciones let
    while peek_keyword(&mut chars, "let") {
        let def = parse_definition(&mut chars)?;
        ir.definitions.push(def);
    }

    // 3. Sección rule
    expect_keyword(&mut chars, "rule")?;
    ir.rule_name = parse_identifier(&mut chars)?;
    expect_char(&mut chars, '=')?;

    // 4. Reglas | regex { acción }
    let mut priority = 0usize;
    while peek_char(&mut chars, '|') {
        consume_char(&mut chars);
        let regex_str = parse_until_brace(&mut chars)?;
        let action    = parse_block(&mut chars)?;
        ir.rules.push(Rule { regex_str, action, priority });
        priority += 1;
    }

    // 5. Trailer opcional
    ir.trailer = parse_block(&mut chars).ok();

    Ok(ir)
}
```

### Entrada / Salida

| | |
|---|---|
| **Entrada** | `&str` — contenido completo del archivo `.yal` |
| **Salida** | `Result<SpecIR, LexError>` — representación estructurada |

---

## Fase 2 — `src/spec/ast.rs`

### Propósito
Define las estructuras de datos que representan la especificación internamente.

```rust
// src/spec/ast.rs

/// Representación interna de toda la especificación .yal
#[derive(Debug, Default)]
pub struct SpecIR {
    pub header:      Option<String>,         // código antes de `let`
    pub definitions: Vec<Definition>,        // macros `let NAME = regex`
    pub rule_name:   String,                 // nombre de la `rule`
    pub rules:       Vec<Rule>,              // pares regex-acción con prioridad
    pub trailer:     Option<String>,         // código al final
}

/// Una definición `let NAME = regex`
#[derive(Debug, Clone)]
pub struct Definition {
    pub name:  String,
    pub regex: String,     // aún sin expandir
}

/// Una regla con su acción y prioridad
#[derive(Debug, Clone)]
pub struct Rule {
    pub regex_str: String,   // regex (puede tener referencias a macros)
    pub action:    String,   // código de acción, ej. "return NUM"
    pub priority:  usize,    // 0 = más prioritaria (primera declarada)
}

/// Regla con regex ya expandida (producida por expand.rs)
#[derive(Debug, Clone)]
pub struct ExpandedRule {
    pub regex_str:  String,
    pub action:     String,
    pub priority:   usize,
    pub token_name: String,   // extraído del action si aplica
}
```

### Entrada / Salida

| | |
|---|---|
| **Entrada** | Datos crudos del parser |
| **Salida** | Tipos usados por todas las fases siguientes |

---

## Fase 3 — `src/spec/expand.rs`

### Propósito
Reemplazar referencias `{MACRO}` por su definición real. Detectar referencias faltantes y ciclos.

### Algoritmo

```
Para cada regla en SpecIR.rules:
    regex_expandida = expandir(regla.regex_str, definiciones)

expandir(s, defs):
    Mientras s contenga {NAME}:
        Buscar NAME en defs
        Si no existe → error "definición no encontrada"
        Sustituir {NAME} por (def expandida de NAME)
        Si ciclo detectado → error "ciclo en definición"
    return s
```

### Implementación

```rust
// src/spec/expand.rs
use std::collections::{HashMap, HashSet};
use crate::spec::ast::{SpecIR, ExpandedRule};
use crate::error::LexError;

pub fn expand(ir: &SpecIR) -> Result<Vec<ExpandedRule>, LexError> {
    // Construir mapa name → regex
    let mut defs: HashMap<String, String> = HashMap::new();
    for d in &ir.definitions {
        defs.insert(d.name.clone(), d.regex.clone());
    }

    // Expandir cada definición (pueden referenciarse entre sí)
    let mut expanded_defs: HashMap<String, String> = HashMap::new();
    for name in defs.keys() {
        let mut visiting = HashSet::new();
        let result = expand_def(name, &defs, &mut expanded_defs, &mut visiting)?;
        expanded_defs.insert(name.clone(), result);
    }

    // Expandir cada regla
    let mut rules = Vec::new();
    for rule in &ir.rules {
        let regex_expanded = substitute(&rule.regex_str, &expanded_defs)?;
        rules.push(ExpandedRule {
            regex_str:  regex_expanded,
            action:     rule.action.clone(),
            priority:   rule.priority,
            token_name: extract_token_name(&rule.action),
        });
    }
    Ok(rules)
}

fn substitute(s: &str, defs: &HashMap<String, String>)
    -> Result<String, LexError>
{
    let mut result = s.to_string();
    // Buscar y reemplazar todas las ocurrencias de {NAME}
    while let Some(start) = result.find('{') {
        let end = result[start..].find('}')
            .ok_or_else(|| LexError::Parse("llave no cerrada".into()))?
            + start;
        let name = &result[start+1..end].to_string();
        let def  = defs.get(name)
            .ok_or_else(|| LexError::UndefinedRef(name.clone()))?;
        result = format!("{}{}{}", &result[..start], def, &result[end+1..]);
    }
    Ok(result)
}
```

### Entrada / Salida

| | |
|---|---|
| **Entrada** | `&SpecIR` con reglas que pueden tener `{MACRO}` |
| **Salida** | `Vec<ExpandedRule>` con regex completamente expandidas |

---

## Fase 4 — `src/regex/parser.rs`

### Propósito
Convertir una regex expandida (texto) en un AST. Respetar precedencia: `*` > concatenación > `|`.

### Gramática (Dragon Book §3.3.3)

```
expr   ::= term ('|' term)*
term   ::= factor factor*
factor ::= atom ('*' | '+' | '?')*
atom   ::= '(' expr ')'
         | '[' clase ']'
         | literal
         | '.'
```

### Implementación (descenso recursivo)

```rust
// src/regex/parser.rs
use crate::regex::ast::RegexNode;
use crate::error::LexError;

pub struct Parser {
    input: Vec<char>,
    pos:   usize,
}

impl Parser {
    pub fn new(s: &str) -> Self { Parser { input: s.chars().collect(), pos: 0 } }

    pub fn parse(&mut self) -> Result<RegexNode, LexError> {
        let node = self.parse_expr()?;
        if self.pos < self.input.len() {
            return Err(LexError::Parse("carácter inesperado al final".into()));
        }
        Ok(node)
    }

    fn parse_expr(&mut self) -> Result<RegexNode, LexError> {
        let mut left = self.parse_term()?;
        while self.peek() == Some('|') {
            self.consume();
            let right = self.parse_term()?;
            left = RegexNode::Union(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<RegexNode, LexError> {
        let mut node = self.parse_factor()?;
        while self.pos < self.input.len()
           && !matches!(self.peek(), Some('|') | Some(')') | None)
        {
            let right = self.parse_factor()?;
            node = RegexNode::Concat(Box::new(node), Box::new(right));
        }
        Ok(node)
    }

    fn parse_factor(&mut self) -> Result<RegexNode, LexError> {
        let mut node = self.parse_atom()?;
        loop {
            match self.peek() {
                Some('*') => { self.consume(); node = RegexNode::Star(Box::new(node)); }
                Some('+') => { self.consume(); node = RegexNode::Plus(Box::new(node)); }
                Some('?') => { self.consume(); node = RegexNode::Optional(Box::new(node)); }
                _         => break,
            }
        }
        Ok(node)
    }

    fn parse_atom(&mut self) -> Result<RegexNode, LexError> {
        match self.peek() {
            Some('(') => {
                self.consume();
                let inner = self.parse_expr()?;
                self.expect(')')?;
                Ok(inner)
            }
            Some('[') => self.parse_char_class(),
            Some('.') => { self.consume(); Ok(RegexNode::AnyChar) }
            Some('\\')=> self.parse_escape(),
            Some(c)   => { self.consume(); Ok(RegexNode::Literal(c)) }
            None      => Err(LexError::Parse("fin inesperado".into())),
        }
    }

    fn parse_char_class(&mut self) -> Result<RegexNode, LexError> {
        self.expect('[')?;
        let negate = self.peek() == Some('^');
        if negate { self.consume(); }
        let mut chars = Vec::new();
        while self.peek() != Some(']') {
            let lo = self.consume_char()?;
            if self.peek() == Some('-') {
                self.consume();
                let hi = self.consume_char()?;
                for c in lo..=hi { chars.push(c); }
            } else {
                chars.push(lo);
            }
        }
        self.expect(']')?;
        Ok(RegexNode::CharClass { chars, negate })
    }

    fn peek(&self) -> Option<char> { self.input.get(self.pos).copied() }
    fn consume(&mut self) -> char  { let c = self.input[self.pos]; self.pos += 1; c }
    fn expect(&mut self, c: char) -> Result<(), LexError> {
        if self.peek() == Some(c) { self.consume(); Ok(()) }
        else { Err(LexError::Parse(format!("se esperaba '{}'", c))) }
    }
    fn consume_char(&mut self) -> Result<char, LexError> {
        self.peek().ok_or(LexError::Parse("eof".into())).map(|_| self.consume())
    }
    fn parse_escape(&mut self) -> Result<RegexNode, LexError> {
        self.consume(); // consume '\'
        let c = self.consume();
        let mapped = match c {
            'n' => '\n', 't' => '\t', 'r' => '\r', _ => c,
        };
        Ok(RegexNode::Literal(mapped))
    }
}

pub fn parse(s: &str) -> Result<RegexNode, LexError> {
    Parser::new(s).parse()
}
```

### Entrada / Salida

| | |
|---|---|
| **Entrada** | `&str` — regex expandida |
| **Salida** | `Result<RegexNode, LexError>` — AST |

---

## Fase 5 — `src/regex/ast.rs`

### Propósito
Define los nodos del árbol AST de expresiones regulares.

```rust
// src/regex/ast.rs

#[derive(Debug, Clone)]
pub enum RegexNode {
    /// Carácter literal, ej. 'a'
    Literal(char),

    /// Cualquier carácter '.'
    AnyChar,

    /// Clase de caracteres [a-z], [^0-9]
    CharClass { chars: Vec<char>, negate: bool },

    /// Unión: a | b
    Union(Box<RegexNode>, Box<RegexNode>),

    /// Concatenación: ab
    Concat(Box<RegexNode>, Box<RegexNode>),

    /// Cierre de Kleene: a*
    Star(Box<RegexNode>),

    /// Una o más: a+  ≡  aa*
    Plus(Box<RegexNode>),

    /// Cero o uno: a?  ≡  a | ε
    Optional(Box<RegexNode>),

    /// Cadena vacía ε (usado internamente por Thompson)
    Epsilon,
}

impl RegexNode {
    /// ¿Puede este nodo generar la cadena vacía?
    pub fn nullable(&self) -> bool {
        match self {
            RegexNode::Epsilon       => true,
            RegexNode::Star(_)       => true,
            RegexNode::Optional(_)   => true,
            RegexNode::Plus(inner)   => inner.nullable(),
            RegexNode::Union(a, b)   => a.nullable() || b.nullable(),
            RegexNode::Concat(a, b)  => a.nullable() && b.nullable(),
            _                        => false,
        }
    }
}
```

---

## Fase 6 — `src/graph/dot.rs`

### Propósito
Convertir el AST de regex, el NFA o el DFA a formato Graphviz DOT.

### Referencia Dragon Book: §3.6.1, §3.6.4

### Implementación

```rust
// src/graph/dot.rs
use std::fs;
use crate::automata::dfa::DFA;
use crate::automata::nfa::NFA;
use crate::regex::ast::RegexNode;

// ─── AST ──────────────────────────────────────────────────────────────────────

pub fn write_ast_dot(path: &str, root: &RegexNode) {
    let mut out = String::from("digraph AST {\n  node [shape=box];\n");
    let mut id = 0usize;
    ast_node(&mut out, root, &mut id);
    out.push('}');
    fs::write(path, out).expect("No se pudo escribir el archivo .dot");
}

fn ast_node(out: &mut String, node: &RegexNode, id: &mut usize) -> usize {
    let my_id = *id; *id += 1;
    let label = match node {
        RegexNode::Literal(c)          => format!("'{}'", c),
        RegexNode::AnyChar             => ".".into(),
        RegexNode::Star(_)             => "*".into(),
        RegexNode::Plus(_)             => "+".into(),
        RegexNode::Optional(_)         => "?".into(),
        RegexNode::Union(_, _)         => "|".into(),
        RegexNode::Concat(_, _)        => "·".into(),
        RegexNode::CharClass{..}       => "[class]".into(),
        RegexNode::Epsilon             => "ε".into(),
    };
    out.push_str(&format!("  n{} [label=\"{}\"];\n", my_id, label));
    match node {
        RegexNode::Star(c) | RegexNode::Plus(c) | RegexNode::Optional(c) => {
            let child = ast_node(out, c, id);
            out.push_str(&format!("  n{} -> n{};\n", my_id, child));
        }
        RegexNode::Union(a, b) | RegexNode::Concat(a, b) => {
            let la = ast_node(out, a, id);
            let lb = ast_node(out, b, id);
            out.push_str(&format!("  n{} -> n{};\n  n{} -> n{};\n", my_id, la, my_id, lb));
        }
        _ => {}
    }
    my_id
}

// ─── DFA ──────────────────────────────────────────────────────────────────────

pub fn write_dfa_file(path: &str, dfa: &DFA) {
    let mut out = String::from("digraph DFA {\n  rankdir=LR;\n  node [shape=circle];\n");
    out.push_str("  __start__ [shape=none label=\"\"];\n");
    out.push_str(&format!("  __start__ -> {};\n", dfa.start));

    for s in &dfa.accept_states {
        out.push_str(&format!("  {} [shape=doublecircle label=\"{}\\n{}\"];\n",
            s, s, dfa.token_of.get(s).map(|t| t.as_str()).unwrap_or("")));
    }
    for (from, sym, to) in &dfa.transitions {
        let label = if *sym == '\0' { "ε".to_string() } else { sym.to_string() };
        out.push_str(&format!("  {} -> {} [label=\"{}\"];\n", from, to, label));
    }
    out.push('}');
    fs::write(path, out).expect("No se pudo escribir el archivo .dot");
}
```

### Entrada / Salida

| | |
|---|---|
| **Entrada** | `&RegexNode`, `&NFA` o `&DFA` |
| **Salida** | Archivo `.dot` (usar `dot -Tpng` para visualizar) |

---

## Fase 7 — `src/automata/nfa.rs`

### Propósito
Construir el NFA usando el **algoritmo de Thompson** (Dragon Book §3.7.4, Algoritmo 3.23).

### Reglas de Thompson

| Regex | NFA construido |
|---|---|
| `ε` | i →ε→ f |
| literal `a` | i →a→ f |
| `r\|s` | nuevo i con ε a NFA(r) y NFA(s); sus aceptaciones van ε a nuevo f |
| `rs` | aceptación de NFA(r) = inicio de NFA(s) |
| `r*` | nuevo i →ε→ inicio(r); aceptación(r) →ε→ inicio(r) y →ε→ nuevo f |

*(Dragon Book pág. 159–163)*

### Estructuras

```rust
// src/automata/nfa.rs
use std::collections::HashMap;
use crate::regex::ast::RegexNode;

pub type StateId = usize;

#[derive(Debug, Clone)]
pub struct NFA {
    pub states:       usize,                           // cantidad de estados
    pub transitions:  Vec<(StateId, Option<char>, StateId)>, // (from, sym, to); None = ε
    pub start:        StateId,
    pub accept_states: HashMap<StateId, (String, usize)>, // state → (token, priority)
}

impl NFA {
    pub fn new() -> Self { NFA { states: 0, transitions: vec![], start: 0, accept_states: HashMap::new() } }
    fn new_state(&mut self) -> StateId { let s = self.states; self.states += 1; s }
    fn add_epsilon(&mut self, from: StateId, to: StateId) { self.transitions.push((from, None, to)); }
    fn add_trans(&mut self, from: StateId, sym: char, to: StateId) { self.transitions.push((from, Some(sym), to)); }
}

/// Construye un NFA desde un RegexNode (Thompson)
pub fn build(nfa: &mut NFA, node: &RegexNode) -> (StateId, StateId) {
    match node {
        RegexNode::Epsilon => {
            let i = nfa.new_state(); let f = nfa.new_state();
            nfa.add_epsilon(i, f); (i, f)
        }
        RegexNode::Literal(c) => {
            let i = nfa.new_state(); let f = nfa.new_state();
            nfa.add_trans(i, *c, f); (i, f)
        }
        RegexNode::AnyChar => {
            // Expandir a unión de todos los printables (simplificado)
            let i = nfa.new_state(); let f = nfa.new_state();
            for c in ' '..='~' { nfa.add_trans(i, c, f); }
            (i, f)
        }
        RegexNode::CharClass { chars, negate } => {
            let i = nfa.new_state(); let f = nfa.new_state();
            let alphabet: Vec<char> = (' '..='~').collect();
            let effective: Vec<char> = if *negate {
                alphabet.into_iter().filter(|c| !chars.contains(c)).collect()
            } else {
                chars.clone()
            };
            for c in effective { nfa.add_trans(i, c, f); }
            (i, f)
        }
        RegexNode::Union(a, b) => {
            let i = nfa.new_state(); let f = nfa.new_state();
            let (as_, af) = build(nfa, a);
            let (bs_, bf) = build(nfa, b);
            nfa.add_epsilon(i, as_); nfa.add_epsilon(i, bs_);
            nfa.add_epsilon(af, f);  nfa.add_epsilon(bf, f);
            (i, f)
        }
        RegexNode::Concat(a, b) => {
            let (as_, af) = build(nfa, a);
            let (bs_, bf) = build(nfa, b);
            nfa.add_epsilon(af, bs_);
            (as_, bf)
        }
        RegexNode::Star(inner) => {
            let i = nfa.new_state(); let f = nfa.new_state();
            let (is_, if_) = build(nfa, inner);
            nfa.add_epsilon(i, is_); nfa.add_epsilon(i, f);
            nfa.add_epsilon(if_, is_); nfa.add_epsilon(if_, f);
            (i, f)
        }
        RegexNode::Plus(inner) => {
            // a+ = a a*
            let node_star = RegexNode::Star(inner.clone());
            let (is_, if_) = build(nfa, inner);
            let (ss, sf) = build(nfa, &node_star);
            nfa.add_epsilon(if_, ss);
            (is_, sf)
        }
        RegexNode::Optional(inner) => {
            // a? = a | ε
            let eps = RegexNode::Epsilon;
            let union = RegexNode::Union(inner.clone(), Box::new(eps));
            build(nfa, &union)
        }
    }
}

/// Construye el NFA global para todas las reglas (unión con prioridad)
pub fn build_global_nfa(rules: &[crate::spec::ast::ExpandedRule]) -> NFA {
    let mut nfa = NFA::new();
    let global_start = nfa.new_state();
    for rule in rules {
        let ast = crate::regex::parser::parse(&rule.regex_str).expect("regex inválida");
        let (rule_start, rule_accept) = build(&mut nfa, &ast);
        nfa.add_epsilon(global_start, rule_start);
        nfa.accept_states.insert(rule_accept, (rule.token_name.clone(), rule.priority));
    }
    nfa.start = global_start;
    nfa
}
```

### Entrada / Salida

| | |
|---|---|
| **Entrada** | `&RegexNode` por regla; `&[ExpandedRule]` para el NFA global |
| **Salida** | `NFA` con estados, transiciones, estado inicial y aceptaciones marcadas con token+prioridad |

---

## Fase 8 — `src/automata/subset.rs`

### Propósito
Convertir el NFA en DFA mediante el **algoritmo de construcción de subconjuntos** (Dragon Book §3.7.1, Algoritmo 3.20).

### Algoritmo

```
DStates = { ε-closure({start}) }   ← estado inicial del DFA
Mientras haya estados no marcados T en DStates:
    marcar T
    Para cada símbolo a en Σ:
        U = ε-closure(move(T, a))
        Si U no está en DStates → agregar U sin marcar
        Dtran[T, a] = U
```

### Implementación

```rust
// src/automata/subset.rs
use std::collections::{HashMap, HashSet, VecDeque};
use crate::automata::nfa::{NFA, StateId};
use crate::automata::dfa::DFA;

type NFASet = Vec<StateId>;

fn epsilon_closure(nfa: &NFA, states: &[StateId]) -> Vec<StateId> {
    let mut closure: HashSet<StateId> = states.iter().cloned().collect();
    let mut stack: Vec<StateId> = states.to_vec();
    while let Some(s) = stack.pop() {
        for (from, sym, to) in &nfa.transitions {
            if *from == s && sym.is_none() && !closure.contains(to) {
                closure.insert(*to);
                stack.push(*to);
            }
        }
    }
    let mut v: Vec<StateId> = closure.into_iter().collect();
    v.sort_unstable();
    v
}

fn move_set(nfa: &NFA, states: &[StateId], sym: char) -> Vec<StateId> {
    let mut result: HashSet<StateId> = HashSet::new();
    for &s in states {
        for (from, label, to) in &nfa.transitions {
            if *from == s && *label == Some(sym) { result.insert(*to); }
        }
    }
    let mut v: Vec<StateId> = result.into_iter().collect();
    v.sort_unstable();
    v
}

pub fn build_dfa(nfa: &NFA) -> DFA {
    // Alfabeto: todos los símbolos usados en el NFA
    let alphabet: Vec<char> = {
        let mut s: HashSet<char> = HashSet::new();
        for (_, sym, _) in &nfa.transitions { if let Some(c) = sym { s.insert(*c); } }
        let mut v: Vec<char> = s.into_iter().collect();
        v.sort_unstable();
        v
    };

    let start_set = epsilon_closure(nfa, &[nfa.start]);
    let mut dfa_states: Vec<NFASet> = vec![start_set.clone()];
    let mut state_index: HashMap<NFASet, StateId> = HashMap::new();
    state_index.insert(start_set, 0);

    let mut dfa = DFA {
        start: 0,
        transitions: vec![],
        accept_states: HashSet::new(),
        token_of: HashMap::new(),
        n_states: 0,
    };

    let mut queue: VecDeque<usize> = VecDeque::from([0]);

    while let Some(t_idx) = queue.pop_front() {
        let t = dfa_states[t_idx].clone();

        // ¿Es estado de aceptación? Elegir el token de menor prioridad (primer declarado)
        let best = t.iter()
            .filter_map(|s| nfa.accept_states.get(s))
            .min_by_key(|(_, pri)| *pri);
        if let Some((token, _)) = best {
            dfa.accept_states.insert(t_idx);
            dfa.token_of.insert(t_idx, token.clone());
        }

        for &sym in &alphabet {
            let moved   = move_set(nfa, &t, sym);
            if moved.is_empty() { continue; }
            let u = epsilon_closure(nfa, &moved);
            let u_idx = if let Some(&idx) = state_index.get(&u) {
                idx
            } else {
                let idx = dfa_states.len();
                state_index.insert(u.clone(), idx);
                dfa_states.push(u);
                queue.push_back(idx);
                idx
            };
            dfa.transitions.push((t_idx, sym, u_idx));
        }
    }

    dfa.n_states = dfa_states.len();
    dfa
}
```

### Entrada / Salida

| | |
|---|---|
| **Entrada** | `&NFA` — autómata no determinista global |
| **Salida** | `DFA` con estados DFA (= conjuntos de estados NFA), transiciones y aceptaciones |

---

## Fase 9 — `src/automata/dfa.rs`

### Propósito
Estructura de datos del DFA. Usada por `subset.rs`, `minimize.rs`, `transition_table.rs` y `graph/dot.rs`.

```rust
// src/automata/dfa.rs
use std::collections::{HashMap, HashSet};
use crate::automata::nfa::StateId;

#[derive(Debug, Clone)]
pub struct DFA {
    pub n_states:     usize,
    pub start:        StateId,
    pub transitions:  Vec<(StateId, char, StateId)>,  // (from, symbol, to)
    pub accept_states: HashSet<StateId>,
    pub token_of:     HashMap<StateId, String>,        // estado → nombre de token
}

impl DFA {
    /// Dado un estado y un símbolo, retorna el siguiente estado o None
    pub fn next(&self, state: StateId, sym: char) -> Option<StateId> {
        self.transitions.iter()
            .find(|(from, s, _)| *from == state && *s == sym)
            .map(|(_, _, to)| *to)
    }

    pub fn is_accepting(&self, state: StateId) -> bool {
        self.accept_states.contains(&state)
    }
}
```

---

## Fase 10 — `src/automata/minimize.rs`

### Propósito
Minimizar el DFA usando el **algoritmo de particiones de Hopcroft** (Dragon Book §3.9.6).

### Algoritmo (particiones)

```
Partición inicial: { estados de aceptación } y { resto }
Refinar mientras haya cambios:
    Para cada partición G y símbolo a:
        Dividir G si los estados en G no van todos al mismo grupo con a
Cada grupo final = un estado del DFA minimizado
```

### Implementación (simplificada)

```rust
// src/automata/minimize.rs
use std::collections::{HashMap, HashSet};
use crate::automata::dfa::DFA;

pub fn minimize(dfa: &DFA) -> DFA {
    // Partición inicial: por token aceptado (cada token distinto = grupo separado)
    let mut partition: Vec<HashSet<usize>> = Vec::new();

    // Agrupar estados de aceptación por token
    let mut by_token: HashMap<String, HashSet<usize>> = HashMap::new();
    for &s in &dfa.accept_states {
        let tok = dfa.token_of.get(&s).cloned().unwrap_or_default();
        by_token.entry(tok).or_default().insert(s);
    }
    for (_, group) in by_token { partition.push(group); }

    // Grupo de estados no aceptantes
    let non_acc: HashSet<usize> = (0..dfa.n_states)
        .filter(|s| !dfa.accept_states.contains(s))
        .collect();
    if !non_acc.is_empty() { partition.push(non_acc); }

    // Alfabeto
    let alphabet: Vec<char> = {
        let mut s: HashSet<char> = HashSet::new();
        for (_, c, _) in &dfa.transitions { s.insert(*c); }
        let mut v: Vec<char> = s.into_iter().collect();
        v.sort_unstable();
        v
    };

    // Refinamiento iterativo
    loop {
        let mut changed = false;
        let mut new_partition: Vec<HashSet<usize>> = Vec::new();

        for group in &partition {
            // Intentar dividir el grupo
            let mut subgroups: Vec<HashSet<usize>> = Vec::new();
            'state: for &s in group {
                for sub in &mut subgroups {
                    let rep = *sub.iter().next().unwrap();
                    // s y rep son equivalentes si van al mismo grupo para todos los símbolos
                    let equiv = alphabet.iter().all(|&a| {
                        let s_next   = dfa.next(s, a).map(|t| group_of(&partition, t));
                        let rep_next = dfa.next(rep, a).map(|t| group_of(&partition, t));
                        s_next == rep_next
                    });
                    if equiv { sub.insert(s); continue 'state; }
                }
                subgroups.push(HashSet::from([s]));
            }
            if subgroups.len() > 1 { changed = true; }
            new_partition.extend(subgroups);
        }

        partition = new_partition;
        if !changed { break; }
    }

    // Construir DFA minimizado
    // Mapear cada estado original al representante de su grupo
    let repr: HashMap<usize, usize> = partition.iter().enumerate()
        .flat_map(|(gi, group)| group.iter().map(move |&s| (s, gi)))
        .collect();

    let new_start = repr[&dfa.start];
    let mut new_trans: Vec<(usize, char, usize)> = Vec::new();
    let mut seen = HashSet::new();
    for &(from, sym, to) in &dfa.transitions {
        let edge = (repr[&from], sym, repr[&to]);
        if seen.insert(edge) { new_trans.push(edge); }
    }

    let new_accept: HashSet<usize> = dfa.accept_states.iter().map(|s| repr[s]).collect();
    let mut new_token: HashMap<usize, String> = HashMap::new();
    for (&s, tok) in &dfa.token_of { new_token.insert(repr[&s], tok.clone()); }

    DFA {
        n_states:     partition.len(),
        start:        new_start,
        transitions:  new_trans,
        accept_states: new_accept,
        token_of:     new_token,
    }
}

fn group_of(partition: &[HashSet<usize>], state: usize) -> usize {
    partition.iter().position(|g| g.contains(&state)).unwrap()
}
```

### Entrada / Salida

| | |
|---|---|
| **Entrada** | `&DFA` — DFA posiblemente con estados redundantes |
| **Salida** | `DFA` minimizado (mismo lenguaje, menos estados) |

---

## Fase 11 — `src/table/transition_table.rs`

### Propósito
Transformar el DFA en una tabla `delta[state][symbol]` de acceso O(1). Núcleo del lexer en tiempo de ejecución.

### Dragon Book §3.6.2, §3.8.3

```rust
// src/table/transition_table.rs
use std::collections::HashMap;
use crate::automata::dfa::DFA;

pub const DEAD: i32 = -1;

#[derive(Debug)]
pub struct TransitionTable {
    /// delta[state][char as usize] = next_state o DEAD
    pub delta:    Vec<Vec<i32>>,
    /// accept[state] = Some(token_name) o None
    pub accept:   Vec<Option<String>>,
    pub start:    usize,
    pub n_states: usize,
    pub alphabet: Vec<char>,
}

impl TransitionTable {
    pub fn next(&self, state: usize, c: char) -> i32 {
        self.delta[state][c as usize]
    }
    pub fn is_accepting(&self, state: usize) -> bool {
        self.accept[state].is_some()
    }
    pub fn token_at(&self, state: usize) -> Option<&str> {
        self.accept[state].as_deref()
    }
}

pub fn build(dfa: &DFA) -> TransitionTable {
    let n = dfa.n_states;
    let mut delta: Vec<Vec<i32>> = vec![vec![DEAD; 128]; n];

    for &(from, sym, to) in &dfa.transitions {
        if (sym as usize) < 128 {
            delta[from][sym as usize] = to as i32;
        }
    }

    let accept: Vec<Option<String>> = (0..n)
        .map(|s| dfa.token_of.get(&s).cloned())
        .collect();

    let alphabet: Vec<char> = {
        let mut s: std::collections::HashSet<char> = std::collections::HashSet::new();
        for (_, c, _) in &dfa.transitions { s.insert(*c); }
        let mut v: Vec<char> = s.into_iter().collect();
        v.sort_unstable();
        v
    };

    TransitionTable { delta, accept, start: dfa.start, n_states: n, alphabet }
}

/// Imprimir la tabla para depuración
pub fn print_table(tt: &TransitionTable) {
    print!("{:>8}", "Estado");
    for &c in &tt.alphabet { print!("{:>6}", c); }
    println!();
    for s in 0..tt.n_states {
        let marker = if tt.is_accepting(s) {
            format!("*{}", tt.token_at(s).unwrap_or(""))
        } else { s.to_string() };
        print!("{:>8}", marker);
        for &c in &tt.alphabet {
            let v = tt.next(s, c);
            if v == DEAD { print!("{:>6}", "-"); }
            else         { print!("{:>6}", v); }
        }
        println!();
    }
}
```

### Entrada / Salida

| | |
|---|---|
| **Entrada** | `&DFA` minimizado |
| **Salida** | `TransitionTable` — acceso O(1) a transiciones y estados de aceptación |

---

## Fase 12 — `src/runtime/simulator.rs`

### Propósito
Simular el lexer sobre texto real. Implementar **maximal munch** (prefijo más largo) y resolución de empate por prioridad.

### Dragon Book §3.6.3 (Alg. 3.18), §3.5.3 (regla del prefijo más largo)

```rust
// src/runtime/simulator.rs
use crate::table::transition_table::TransitionTable;

#[derive(Debug, Clone)]
pub struct Token {
    pub kind:    String,   // nombre del token, ej. "NUM", "IDENT"
    pub lexeme:  String,   // texto que coincidió
    pub line:    usize,
    pub col:     usize,
}

#[derive(Debug)]
pub enum LexResult {
    Token(Token),
    Error { lexeme: char, line: usize, col: usize },
    EOF,
}

pub struct Simulator<'a> {
    table:  &'a TransitionTable,
    input:  Vec<char>,
    pos:    usize,
    line:   usize,
    col:    usize,
}

impl<'a> Simulator<'a> {
    pub fn new(table: &'a TransitionTable, input: &str) -> Self {
        Simulator { table, input: input.chars().collect(), pos: 0, line: 1, col: 1 }
    }

    /// Retorna el siguiente token (maximal munch)
    pub fn next_token(&mut self) -> LexResult {
        if self.pos >= self.input.len() { return LexResult::EOF; }

        let mut state = self.table.start as i32;
        let start_pos = self.pos;
        let start_line = self.line;
        let start_col  = self.col;

        // Último estado de aceptación visto (para maximal munch)
        let mut last_accept_pos:   Option<usize> = None;
        let mut last_accept_token: Option<String> = None;

        // ─── Algoritmo 3.18 del Dragon Book ───────────────────────────────────
        while self.pos < self.input.len() {
            let c = self.input[self.pos];
            let next = self.table.next(state as usize, c);
            if next == crate::table::transition_table::DEAD { break; }
            state = next;
            self.pos += 1;
            if c == '\n' { self.line += 1; self.col = 1; }
            else         { self.col  += 1; }

            if self.table.is_accepting(state as usize) {
                last_accept_pos   = Some(self.pos);
                last_accept_token = self.table.token_at(state as usize)
                    .map(String::from);
            }
        }

        // Aplicar maximal munch: retroceder a la última aceptación
        if let Some(accept_pos) = last_accept_pos {
            self.pos = accept_pos;
            let lexeme: String = self.input[start_pos..accept_pos].iter().collect();
            LexResult::Token(Token {
                kind:   last_accept_token.unwrap_or_default(),
                lexeme,
                line:   start_line,
                col:    start_col,
            })
        } else {
            // Sin aceptación: error léxico
            let bad = self.input[start_pos];
            self.pos = start_pos + 1;
            LexResult::Error { lexeme: bad, line: start_line, col: start_col }
        }
    }

    /// Tokenizar todo el input
    pub fn tokenize(&mut self) -> (Vec<Token>, Vec<String>) {
        let mut tokens = Vec::new();
        let mut errors = Vec::new();
        loop {
            match self.next_token() {
                LexResult::Token(t) => tokens.push(t),
                LexResult::Error { lexeme, line, col } =>
                    errors.push(format!("Error línea {}:{} — carácter '{}'", line, col, lexeme)),
                LexResult::EOF => break,
            }
        }
        (tokens, errors)
    }
}
```

### Entrada / Salida

| | |
|---|---|
| **Entrada** | `&TransitionTable`, `&str` texto a analizar |
| **Salida** | `Vec<Token>` (tokens encontrados) y `Vec<String>` (errores léxicos) |

---

## Fase 13 — `src/codegen/rust_codegen.rs`

### Propósito
Generar el archivo fuente `generated/lexer.rs` que implementa el lexer completo de forma compilada, sin depender de las estructuras internas.

### Dragon Book §3.8.3, §8.1

```rust
// src/codegen/rust_codegen.rs
use std::fs;
use crate::table::transition_table::TransitionTable;
use crate::spec::ast::ExpandedRule;

pub fn emit_file(path: &str, tt: &TransitionTable, rules: &[ExpandedRule]) {
    let mut code = String::new();

    // ── Cabecera ──────────────────────────────────────────────────────────────
    code.push_str("// Generado automáticamente por YALex — NO editar\n\n");

    // ── Tabla de transición estática ──────────────────────────────────────────
    let n = tt.n_states;
    code.push_str(&format!("const N_STATES: usize = {};\n", n));
    code.push_str("const DEAD: i32 = -1;\n\n");
    code.push_str(&format!(
        "static DELTA: [[i32; 128]; {}] = [\n", n));
    for s in 0..n {
        code.push_str("    [");
        let row: Vec<String> = (0..128u8)
            .map(|c| tt.delta[s][c as usize].to_string())
            .collect();
        code.push_str(&row.join(", "));
        code.push_str("],\n");
    }
    code.push_str("];\n\n");

    // ── Tokens de aceptación ─────────────────────────────────────────────────
    code.push_str(&format!(
        "static ACCEPT: [Option<&'static str>; {}] = [\n", n));
    for s in 0..n {
        match tt.token_at(s) {
            Some(tok) => code.push_str(&format!("    Some(\"{}\"),\n", tok)),
            None      => code.push_str("    None,\n"),
        }
    }
    code.push_str("];\n\n");

    // ── Tipo Token ────────────────────────────────────────────────────────────
    code.push_str("#[derive(Debug, Clone)]\npub struct Token {\n");
    code.push_str("    pub kind: &'static str,\n");
    code.push_str("    pub lexeme: String,\n");
    code.push_str("    pub line: usize,\n    pub col: usize,\n}\n\n");

    // ── Función next_token ────────────────────────────────────────────────────
    code.push_str(&format!(
        "pub fn next_token(input: &[char], pos: &mut usize, line: &mut usize, col: &mut usize)\n    -> Option<Result<Token, String>>\n{{\n"
    ));
    code.push_str(&format!("    if *pos >= input.len() {{ return None; }}\n"));
    code.push_str(&format!("    let mut state: i32 = {};\n", tt.start));
    code.push_str("    let start = *pos;\n");
    code.push_str("    let (mut last_pos, mut last_tok) = (None::<usize>, None::<&str>);\n\n");
    code.push_str("    while *pos < input.len() {\n");
    code.push_str("        let c = input[*pos] as usize;\n");
    code.push_str("        if c >= 128 { break; }\n");
    code.push_str("        let next = DELTA[state as usize][c];\n");
    code.push_str("        if next == DEAD { break; }\n");
    code.push_str("        state = next; *pos += 1;\n");
    code.push_str("        if input[*pos - 1] == '\\n' { *line += 1; *col = 1; } else { *col += 1; }\n");
    code.push_str("        if let Some(tok) = ACCEPT[state as usize] {\n");
    code.push_str("            last_pos = Some(*pos); last_tok = Some(tok);\n        }\n    }\n\n");
    code.push_str("    if let Some(p) = last_pos {\n");
    code.push_str("        *pos = p;\n");
    code.push_str("        let lexeme: String = input[start..p].iter().collect();\n");
    code.push_str("        Some(Ok(Token { kind: last_tok.unwrap(), lexeme, line: *line, col: *col }))\n");
    code.push_str("    } else {\n");
    code.push_str("        let bad = input[start]; *pos = start + 1;\n");
    code.push_str("        Some(Err(format!(\"Error léxico línea {}:{} — '{}'\", line, col, bad)))\n");
    code.push_str("    }\n}\n\n");

    // ── Función tokenize ──────────────────────────────────────────────────────
    code.push_str("pub fn tokenize(src: &str) -> (Vec<Token>, Vec<String>) {\n");
    code.push_str("    let chars: Vec<char> = src.chars().collect();\n");
    code.push_str("    let (mut pos, mut line, mut col) = (0, 1, 1);\n");
    code.push_str("    let mut tokens = Vec::new(); let mut errors = Vec::new();\n");
    code.push_str("    while let Some(res) = next_token(&chars, &mut pos, &mut line, &mut col) {\n");
    code.push_str("        match res {\n");
    code.push_str("            Ok(t)  => tokens.push(t),\n");
    code.push_str("            Err(e) => errors.push(e),\n");
    code.push_str("        }\n    }\n    (tokens, errors)\n}\n");

    // ── Trailer del .yal ──────────────────────────────────────────────────────
    // (acciones de usuario ya embebidas en ACCEPT — para acciones complejas
    //  se puede generar un match sobre kind aquí)

    fs::write(path, code).expect("No se pudo escribir el lexer generado");
}
```

### Entrada / Salida

| | |
|---|---|
| **Entrada** | `&TransitionTable`, `&[ExpandedRule]` (para acciones) |
| **Salida** | `generated/lexer.rs` — archivo fuente Rust compilable |

---

## Fase 14 — `src/error.rs`

### Propósito
Centralizar todos los errores del compilador en un tipo único.

```rust
// src/error.rs
use std::fmt;

#[derive(Debug)]
pub enum LexError {
    Parse(String),
    UndefinedRef(String),
    CyclicDefinition(String),
    InvalidTransition { state: usize, symbol: char },
    IO(std::io::Error),
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexError::Parse(msg)              => write!(f, "Error de parseo: {}", msg),
            LexError::UndefinedRef(name)      => write!(f, "Referencia no definida: {{{}}}", name),
            LexError::CyclicDefinition(name)  => write!(f, "Ciclo en definición: {}", name),
            LexError::InvalidTransition{s,sym}=> write!(f, "Transición inválida desde {} con '{}'", s, sym),
            LexError::IO(e)                   => write!(f, "Error de I/O: {}", e),
        }
    }
}

impl From<std::io::Error> for LexError {
    fn from(e: std::io::Error) -> Self { LexError::IO(e) }
}
```

---

## Estructura de archivos del proyecto

```
yalex/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── error.rs
│   ├── spec/
│   │   ├── mod.rs
│   │   ├── parser.rs          ← Fase 1
│   │   ├── ast.rs             ← Fase 2
│   │   └── expand.rs          ← Fase 3
│   ├── regex/
│   │   ├── mod.rs
│   │   ├── parser.rs          ← Fase 4
│   │   └── ast.rs             ← Fase 5
│   ├── graph/
│   │   ├── mod.rs
│   │   └── dot.rs             ← Fase 6
│   ├── automata/
│   │   ├── mod.rs
│   │   ├── nfa.rs             ← Fase 7
│   │   ├── subset.rs          ← Fase 8
│   │   ├── dfa.rs             ← Fase 9
│   │   └── minimize.rs        ← Fase 10
│   ├── table/
│   │   ├── mod.rs
│   │   └── transition_table.rs ← Fase 11
│   ├── runtime/
│   │   ├── mod.rs
│   │   └── simulator.rs       ← Fase 12
│   └── codegen/
│       ├── mod.rs
│       └── rust_codegen.rs    ← Fase 13
├── generated/
│   └── lexer.rs               ← salida final
└── output/
    ├── nfa.dot
    └── dfa.dot
```

---

## Tabla de Referencias al Dragon Book

| Fase | Módulo | Secciones clave |
|---|---|---|
| 1–3 | `spec/` | — (diseño propio de formato YALex) |
| 4–5 | `regex/parser.rs`, `regex/ast.rs` | §3.3.3 Expresiones regulares, §3.3.5 Extensiones |
| 6 | `graph/dot.rs` | §3.6.1 NFA, §3.6.4 DFA, §3.6.2 Tablas de transición |
| 7 | `automata/nfa.rs` | §3.7.4 Alg. 3.23 (Thompson NFA) |
| 8 | `automata/subset.rs` | §3.7.1 Alg. 3.20 (Subset construction) |
| 9 | `automata/dfa.rs` | §3.6.4 DFA definición formal |
| 10 | `automata/minimize.rs` | §3.9.6 Minimización de DFA (Hopcroft) |
| 11 | `table/transition_table.rs` | §3.6.2, §3.8.3 DFAs para lexers |
| 12 | `runtime/simulator.rs` | §3.6.3 Alg. 3.18 (simulación DFA), §3.5.3 (maximal munch) |
| 13 | `codegen/rust_codegen.rs` | §3.8.3, §8.1 Diseño del generador de código, §8.3 Direccionamiento |