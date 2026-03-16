// Expansión de macros en los patrones de las reglas.
// Las macros tienen la forma {nombre} y son reemplazadas por
// la expresión regular correspondiente de la sección "let".
//
// Ejemplo:
//   let digit = 0|1|2|3|4|5|6|7|8|9
//   {digit}+  =>  (0|1|2|3|4|5|6|7|8|9)+

use std::collections::HashMap;

use crate::spec::ast::{Rule, SpecIR};

/// Regla con su patrón ya completamente expandido (sin referencias a macros).
#[derive(Debug, Clone)]
pub struct ExpandedRule {
    pub pattern_expanded: String,
    pub action_code: String,
    pub priority: usize,
}

/// Recorre todas las reglas del SpecIR y expande sus macros.
/// Las definiciones se ordenan de mayor a menor longitud de nombre para evitar
/// que un nombre corto reemplace parcialmente a uno más largo (p.ej. "id" vs "id2").
pub fn expand_definitions(spec: &SpecIR) -> Vec<ExpandedRule> {
    // 1. Recoger los defs
    let mut defs: HashMap<String, String> = spec
        .definitions
        .iter()
        .map(|d| (d.name.clone(), d.regex.clone()))
        .collect();

    // 2. Expandir las definiciones entre sí hasta que no haya más llaves 
    //    (expansión recursiva de macros dentro de macros)
    let mut changed = true;
    let mut iterations = 0;
    while changed && iterations < 100 {
        changed = false;
        let mut new_defs = defs.clone();
        
        let mut sorted_keys: Vec<String> = defs.keys().cloned().collect();
        sorted_keys.sort_by(|a, b| b.len().cmp(&a.len()));

        for (k, v) in defs.iter() {
            let mut expanded_v = v.clone();
            for sub_k in &sorted_keys {
                let placeholder = format!("{{{}}}", sub_k);
                if expanded_v.contains(&placeholder) {
                    let replacement = format!("({})", defs[sub_k]);
                    expanded_v = expanded_v.replace(&placeholder, &replacement);
                    changed = true;
                }
            }
            new_defs.insert(k.clone(), expanded_v);
        }
        defs = new_defs;
        iterations += 1;
    }

    // Ordenar por longitud descendente para evitar reemplazos parciales
    let mut sorted_names: Vec<&String> = defs.keys().collect();
    sorted_names.sort_by(|a, b| b.len().cmp(&a.len()));

    spec.rules
        .iter()
        .map(|rule| expand_rule(rule, &defs, &sorted_names))
        .collect()
}

/// Expande todas las referencias {macro} en el patrón de una sola regla.
/// Aplica sustituciones en orden de mayor a menor longitud de nombre.
fn expand_rule(
    rule: &Rule,
    defs: &HashMap<String, String>,
    sorted_names: &[&String],
) -> ExpandedRule {
    let mut expanded = rule.pattern_raw.clone();

    for name in sorted_names {
        let placeholder = format!("{{{}}}", name);
        // Cada macro se envuelve en paréntesis para preservar precedencia
        let replacement = format!("({})", defs[*name]);
        expanded = expanded.replace(&placeholder, &replacement);
    }

    ExpandedRule {
        pattern_expanded: expanded,
        action_code: rule.action_code.clone(),
        priority: rule.priority,
    }
}