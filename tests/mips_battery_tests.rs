//! Fase 6 de la generación de MIPS (ver `PLAN_MIPS.md`): cada programa imprime lo
//! que declara en sus `// salida:`, en el intérprete y en MARS con los dos
//! asignadores.
#[allow(dead_code)]
mod common;

use lexer_generator::codigo_objetivo::{generate_with, Allocator};
use lexer_generator::intermedio::vm::Vm;
use std::fs;
use std::path::PathBuf;

const CARPETAS: &[&str] = &["workspace/mips/ok", "workspace/tac/ok"];

fn programas() -> Vec<PathBuf> {
    let mut rutas: Vec<_> = CARPETAS
        .iter()
        .flat_map(|carpeta| fs::read_dir(carpeta).unwrap_or_else(|e| panic!("{carpeta}: {e}")))
        .map(|entrada| entrada.unwrap().path())
        .filter(|ruta| ruta.extension().is_some_and(|ext| ext == "cps"))
        .collect();
    rutas.sort();
    rutas
}

/// Las líneas `// salida:`. Conserva los espacios del inicio: la pirámide los usa.
fn salida_declarada(fuente: &str) -> String {
    fuente.lines().filter_map(|linea| linea.strip_prefix("// salida: ")).map(|linea| format!("{linea}\n")).collect()
}

#[test]
fn cada_programa_imprime_lo_declarado_en_el_interprete_y_en_mars() {
    let compilar = common::compilador();
    let mut fallos = Vec::new();

    for ruta in programas() {
        let ruta = ruta.display().to_string();
        let fuente = fs::read_to_string(&ruta).unwrap();
        let esperado = salida_declarada(&fuente);
        assert!(!esperado.is_empty(), "{ruta} no declara // salida:");

        let programa = match compilar(&fuente) {
            Ok(programa) => programa,
            Err(error) => {
                fallos.push(format!("{ruta}: {error}"));
                continue;
            }
        };

        match Vm::with_max_steps(&programa, 5_000_000).run() {
            Ok(resultado) if resultado.output == esperado => {}
            Ok(resultado) => fallos.push(format!("{ruta}: el intérprete dio {:?} y se esperaba {esperado:?}", resultado.output)),
            Err(error) => fallos.push(format!("{ruta}: el intérprete falló: {error}")),
        }

        for (etiqueta, asignador) in [("getReg", Allocator::GetReg), ("pila", Allocator::Pila)] {
            let asm = match generate_with(&programa, asignador) {
                Ok(mips) => mips.to_string(),
                Err(errores) => {
                    fallos.push(format!("{ruta} ({etiqueta}): {errores:?}"));
                    continue;
                }
            };
            if let Some(obtenido) = common::correr_mars(&asm) {
                if obtenido != (esperado.clone(), 0) {
                    fallos.push(format!("{ruta} ({etiqueta}): MARS dio {obtenido:?} y se esperaba {esperado:?}"));
                }
            }
        }
    }

    assert!(fallos.is_empty(), "{} fallos:\n{}", fallos.len(), fallos.join("\n"));
}
