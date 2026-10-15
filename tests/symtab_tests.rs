mod common;

use common::compilar;
use lexer_generator::intermedio::gen::extend_layout;
use lexer_generator::intermedio::symtab::{ExtendedSymbolTable, SymbolEntry};

const TABLA: &str = "let x: integer = 10;\n\
                     let nombre: string = \"a\";\n\
                     function contador(paso: integer, b: boolean): integer {\n\
                       let total: integer = 0;\n\
                       function sumar(): integer { total = total + paso; return total; }\n\
                       if (b) { let extra: integer = 1; total = extra; }\n\
                       return sumar();\n\
                     }\n\
                     class Animal { var edad: integer = 0; function hablar(): string { return \"...\"; } }\n\
                     class Perro : Animal { var raza: string = \"\"; function hablar(): string { return \"guau\"; } }\n";

fn symbol<'a>(table: &'a ExtendedSymbolTable, name: &str, scope: &str) -> &'a SymbolEntry {
    table
        .symbols
        .iter()
        .find(|symbol| symbol.name == name && symbol.scope == scope)
        .unwrap_or_else(|| panic!("falta {name} en {scope}: {:#?}", table.symbols))
}

#[test]
fn presenta_offsets_frames_y_vtables_del_ejemplo_del_plan() {
    let (program, mut analysis) = compilar(TABLA);
    extend_layout(&mut analysis.layout, &program);
    let table = ExtendedSymbolTable::build(&analysis, &program);

    let x = symbol(&table, "x", "Global");
    assert_eq!((x.area.as_deref(), x.offset, x.size), (Some("G"), Some(0), Some(4)));
    let nombre = symbol(&table, "nombre", "Global");
    assert_eq!((nombre.area.as_deref(), nombre.offset, nombre.size), (Some("G"), Some(4), Some(4)));

    let paso = symbol(&table, "paso", "contador");
    assert_eq!((paso.area.as_deref(), paso.offset, paso.size), (Some("fp"), Some(16), Some(4)));
    let b = symbol(&table, "b", "contador");
    assert_eq!((b.area.as_deref(), b.offset, b.size), (Some("fp"), Some(20), Some(1)));
    let total = table
        .symbols
        .iter()
        .find(|entry| entry.name == "total")
        .unwrap();
    assert_eq!(total.offset, Some(-16));
    assert!(total.scope.contains("contador"));
    assert_eq!(
        table
            .symbols
            .iter()
            .find(|entry| entry.name == "extra")
            .and_then(|entry| entry.offset),
        Some(-12)
    );

    let contador = symbol(&table, "contador", "Global");
    assert_eq!(contador.label.as_deref(), Some("contador"));
    assert_eq!(contador.nesting_level, Some(1));
    assert_eq!(contador.size, Some(16));
    let sumar = table.symbols.iter().find(|entry| entry.name == "sumar").unwrap();
    assert_eq!(sumar.label.as_deref(), Some("contador.sumar"));
    assert_eq!(sumar.nesting_level, Some(2));

    let contador_frame = table.frames.iter().find(|frame| frame.owner == "contador").unwrap();
    assert_eq!(contador_frame.base_frame_size, 16);
    assert_eq!(contador_frame.temp_slots, program.functions.iter().find(|f| f.name == "contador").unwrap().max_temps);
    assert_eq!(
        contador_frame.total_frame_size,
        contador_frame.base_frame_size + contador_frame.temp_slots * 8
    );

    let animal = table.classes.iter().find(|class| class.name == "Animal").unwrap();
    assert_eq!(animal.instance_size, 8);
    assert_eq!(animal.fields[0].name, "edad");
    assert_eq!(animal.fields[0].offset, 4);
    assert_eq!(animal.vtable, vec!["Animal.hablar"]);

    let perro = table.classes.iter().find(|class| class.name == "Perro").unwrap();
    assert_eq!(perro.instance_size, 12);
    assert_eq!(perro.fields[0].name, "edad");
    assert_eq!(perro.fields[0].offset, 4);
    assert_eq!(perro.fields[1].name, "raza");
    assert_eq!(perro.fields[1].offset, 8);
    assert_eq!(perro.vtable, vec!["Perro.hablar"]);
}

#[test]
fn ofrece_la_misma_vista_como_texto_y_json() {
    let (program, mut analysis) = compilar(TABLA);
    extend_layout(&mut analysis.layout, &program);
    let table = ExtendedSymbolTable::build(&analysis, &program);

    let text = table.to_text();
    assert!(text.contains("x | variable | integer | Global | G | +0 | 4"));
    assert!(text.contains("contador | 1 | 16 |"));
    assert!(text.contains("class Animal: 8 bytes; vtable vt_Animal [Animal.hablar]"));

    let json = table.to_json();
    assert_eq!(json["symbols"].as_array().unwrap().len(), table.symbols.len());
    let perro = json["classes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|class| class["name"] == "Perro")
        .unwrap();
    assert_eq!(perro["instance_size"], 12);
    assert_eq!(perro["vtable"][0], "Perro.hablar");

    let pretty = table.to_json_pretty();
    assert!(pretty.contains("\"total_frame_size\""));
}
