
// almacenamiento de datos ordenados 

#[derive(Debug, Clone)]
// SpecIR: almacena/guarda todo el archivo ordenado
pub struct SpecIR {
    pub header: Option<String>,
    pub definitions: Vec<Definition>,
    pub rules: Vec<Rule>,
    pub trailer: Option<String>,
}

// Definition: guarda cada definición let
#[derive(Debug, Clone)]
pub struct Definition {
    pub name: String,
    pub regex: String,
}

// Rule: guarda cada regla con su acción y prioridad
#[derive(Debug, Clone)]
pub struct Rule {
    pub pattern_raw: String,
    pub action_code: String,
    pub priority: usize,
}