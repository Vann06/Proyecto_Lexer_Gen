
// reemplazo de macros 
// DIGIT = [0-9]
// {DIGIT}* => ([0-9])*
  
use std::collections::HashMap;

use crate::spec::ast::{Rule, SpecIR};

#[derive(Debug, Clone)]
pub struct ExpandedRule {
    pub pattern_expanded: String,
    pub action_code: String,
    pub priority: usize,
}

pub fn expand_definitions(spec: &SpecIR) -> Vec<ExpandedRule> {
    let defs: HashMap<String, String> = spec
        .definitions
        .iter()
        .map(|d| (d.name.clone(), d.regex.clone()))
        .collect();

    spec.rules
        .iter()
        .map(|rule| expand_rule(rule, &defs))
        .collect()
}

fn expand_rule(rule: &Rule, defs: &HashMap<String, String>) -> ExpandedRule {
    let mut expanded = rule.pattern_raw.clone();

    for (name, value) in defs {
        let placeholder = format!("{{{}}}", name);
        let replacement = format!("({})", value);
        expanded = expanded.replace(&placeholder, &replacement);
    }

    ExpandedRule {
        pattern_expanded: expanded,
        action_code: rule.action_code.clone(),
        priority: rule.priority,
    }
}