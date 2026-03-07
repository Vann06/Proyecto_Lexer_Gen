
// almacenamiento de datos ordenados 

#[derive(Debug, Clone)]
pub struct SpecIR {
    pub header: Option<String>,
    pub definitions: Vec<Definition>,
    pub rules: Vec<Rule>,
    pub trailer: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Definition {
    pub name: String,
    pub regex: String,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub pattern_raw: String,
    pub action_code: String,
    pub priority: usize,
}