// Asignación de almacenamiento (capítulo 7 del libro del dragón, "Run-Time
// Environments"): a partir de los tipos que ya dejó `semantico`, calcula
// dónde vive cada símbolo — offset dentro de un marco de activación, o
// dentro de una instancia de objeto — y cuánto ocupa.
//
// NO forma parte del recorrido semántico: `analyzer` sigue sin nombrar
// producciones, y este módulo tampoco lo hace. Lo único que consume de la
// gramática es lo que `symbols`/`scopes` ya exponían: `Symbol.ty`,
// `Symbol.kind`, `Symbol.decl_index` y el árbol de ámbitos.
//
// Restricción de diseño: los ANCHOS son un hecho de la MÁQUINA DESTINO, no
// de la gramática. Por eso `TargetLayout` es una tabla de constantes en
// código (no una directiva `%` del `.yalp`) — la misma gramática debe poder
// compilarse a dos destinos sin tocar el `.yalp`. `analyzer` lo invoca al
// cerrar cada ámbito de función/bloque y, al final del recorrido, para el
// Global y las clases (ver `ARQUITECTURA.md` §8).

use std::collections::{HashMap, HashSet};

use super::scopes::Scope;
use super::symbols::{StorageInfo, Symbol, SymbolKind};
use super::types::Type;

/// Los anchos y offsets base de la máquina destino. `Default` da MIPS
/// (SPIM/MARS): palabra de 4 bytes, marco creciente hacia abajo con `$fp`
/// apuntando al enlace de control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetLayout {
    /// Ancho de palabra — también la alineación por defecto de una ranura de
    /// marco (todo local/parámetro redondea su tamaño hacia arriba a esto).
    pub word: usize,
    /// Ancho de una referencia: lo que ocupan `Str`, las colecciones y
    /// cualquier `Named` — nunca el objeto/dato completo.
    pub pointer: usize,
    pub int: usize,
    pub float: usize,
    pub bool_: usize,
    /// Offset del enlace de acceso respecto a `$fp` (§7.3 del libro): el
    /// `$fp` del marco de la función que ENCIERRA estáticamente a esta. Va en
    /// TODOS los marcos, aunque una función de nivel 1 no lo use — una
    /// convención de llamada uniforme es más simple que un caso especial.
    pub access_link: isize,
    /// Offset del PRIMER parámetro respecto a `$fp` (positivo, crece hacia
    /// arriba). `3*word`: entre `$fp` y los parámetros están el enlace de
    /// control (`$fp+0`), el valor de retorno (`$fp+word`) y el enlace de
    /// acceso (`$fp+2*word`).
    pub param_base: isize,
    /// Offset del PRIMER local respecto a `$fp` (negativo, crece hacia
    /// abajo). `-2*word` porque entre `$fp` y los locales está el estado
    /// salvado / `$ra` (`$fp-word`).
    pub local_base: isize,
}

impl Default for TargetLayout {
    fn default() -> Self {
        TargetLayout { word: 4, pointer: 4, int: 4, float: 8, bool_: 1, access_link: 8, param_base: 12, local_base: -8 }
    }
}

/// El ancho de un tipo, o la confesión honesta de que todavía no se puede
/// calcular.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    Known(usize),
    /// El tipo es (o contiene) `Type::Unknown`. Nunca se le asigna 0 — eso
    /// correría en silencio todos los offsets siguientes del mismo marco.
    /// Quien reciba esto debe SALTAR el símbolo, no adivinar.
    Unresolved,
}

fn natural_align(size: usize, word: usize) -> usize {
    size.min(word.max(1)).max(1)
}

fn align_up(offset: usize, align: usize) -> usize {
    if align <= 1 {
        return offset;
    }
    let rem = offset % align;
    if rem == 0 { offset } else { offset + (align - rem) }
}

/// Ancho de `ty` en la máquina `target`.
///
/// Decisiones (documentadas en el plan de implementación, no repetidas acá
/// por cada rama): `Str`/`Array`/`Map`/`Set`/`Named` son referencias de
/// tamaño fijo — `new` ya tiene semántica de referencia y las colecciones
/// son de tamaño dinámico, así que cargarlas inline no tendría sentido.
/// `Tuple` es la única agregación por VALOR (aridad fija) — suma sus campos
/// con alineación natural y relleno, como lo haría un `struct` de C.
pub fn width_of(ty: &Type, target: &TargetLayout) -> Width {
    match ty {
        Type::Int => Width::Known(target.int),
        Type::Float => Width::Known(target.float),
        Type::Bool => Width::Known(target.bool_),
        Type::Void => Width::Known(0),
        Type::Str | Type::Array(_) | Type::Map(_, _) | Type::Set(_) | Type::Named(_) => {
            Width::Known(target.pointer)
        }
        Type::Tuple(items) => {
            let mut offset = 0usize;
            let mut max_align = 1usize;
            for item in items {
                let w = match width_of(item, target) {
                    Width::Known(w) => w,
                    Width::Unresolved => return Width::Unresolved,
                };
                let align = natural_align(w, target.word);
                offset = align_up(offset, align);
                offset += w;
                max_align = max_align.max(align);
            }
            Width::Known(align_up(offset, max_align))
        }
        Type::Unknown => Width::Unresolved,
    }
}

/// Si el layout de un ámbito o de una clase quedó completo, o si algún
/// símbolo se tuvo que saltar por tener tipo `Unknown`. Una fase de TAC
/// debe rechazar generar direcciones sobre un marco `Incomplete` en vez de
/// emitir una que sea plausible pero falsa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutStatus {
    Complete,
    Incomplete,
}

enum FrameKind {
    /// Marco de una función: parámetros positivos, locales negativos.
    Function,
    /// Área de datos estáticos (el ámbito Global): un solo carril positivo
    /// desde 0, sin parámetros.
    Static,
}

/// Reparte offsets dentro de UN marco (o del área estática). Se comparte
/// entre el ámbito de una función y todos sus bloques anidados —eso es lo
/// que hace que los locales de un `if`/`while` ACUMULEN en el mismo marco en
/// vez de solaparse con los de otro bloque hermano, cerrando el hueco de
/// "¿qué locales caen en el marco de qué función?" una vez que
/// `analyzer::exit` usa el `FrameAllocator` de la función más interna
/// abierta para cada bloque que cierra.
pub struct FrameAllocator {
    kind: FrameKind,
    param_bytes: usize,
    local_bytes: usize,
    param_base: isize,
    local_base: isize,
    /// La máquina para la que se reparte: `allocate_scope` la necesita para
    /// medir cada símbolo con los MISMOS anchos con que se creó el asignador.
    target: TargetLayout,
}

impl FrameAllocator {
    pub fn new_function(target: &TargetLayout) -> Self {
        FrameAllocator {
            kind: FrameKind::Function,
            param_bytes: 0,
            local_bytes: 0,
            param_base: target.param_base,
            local_base: target.local_base,
            target: *target,
        }
    }

    /// Para el ámbito Global: datos estáticos, positivos desde 0.
    pub fn new_static(target: &TargetLayout) -> Self {
        FrameAllocator {
            kind: FrameKind::Static,
            param_bytes: 0,
            local_bytes: 0,
            param_base: 0,
            local_base: 0,
            target: *target,
        }
    }

    /// Reserva una ranura de `size_bytes` para un parámetro. Offset positivo,
    /// creciente. En un asignador `Static` no hay carril de parámetros: un
    /// `Parameter` declarado fuera de toda función (una gramática que lo
    /// permita, o un árbol de prueba con un `param` suelto) recibe una ranura
    /// estática común, como cualquier global, en vez de un offset relativo a
    /// un `$fp` que no existe.
    pub fn alloc_param(&mut self, size_bytes: usize) -> isize {
        if matches!(self.kind, FrameKind::Static) {
            return self.alloc_local(size_bytes);
        }
        let slot = align_up(size_bytes, self.target.word);
        let offset = self.param_base + self.param_bytes as isize;
        self.param_bytes += slot;
        offset
    }

    /// Reserva una ranura de `size_bytes` para un local (o, en modo
    /// `Static`, para una variable global). El offset devuelto es la
    /// dirección BAJA de la ranura.
    pub fn alloc_local(&mut self, size_bytes: usize) -> isize {
        let slot = align_up(size_bytes, self.target.word);
        let offset = match self.kind {
            FrameKind::Function => self.local_base - self.local_bytes as isize - slot as isize,
            FrameKind::Static => self.local_base + self.local_bytes as isize,
        };
        self.local_bytes += slot;
        offset
    }

    /// Total de bytes que hay que reservar: para una función, el área fija
    /// del marco (enlace de control + estado salvado) más todos los locales
    /// acumulados; para el área estática, solo lo acumulado.
    pub fn size(&self) -> usize {
        match self.kind {
            FrameKind::Function => self.local_base.unsigned_abs() + self.local_bytes,
            FrameKind::Static => self.local_bytes,
        }
    }
}

/// Asigna offset a cada símbolo `Variable`/`Parameter` declarado DIRECTAMENTE
/// en `scope`, en orden de declaración (`Symbol::decl_index`, nunca el orden
/// del `HashMap` que los respalda). El resto de `SymbolKind` (funciones,
/// clases, structs anidados) no ocupa espacio de marco — los saltea.
///
/// `alloc` se pasa por fuera y no se crea acá a propósito: es lo que permite
/// que varios `Scope` (una función y sus bloques anidados) compartan el
/// mismo marco.
pub fn allocate_scope(scope: &mut Scope, alloc: &mut FrameAllocator) -> LayoutStatus {
    let mut order: Vec<(String, usize)> = scope.symbols().map(|s| (s.name.clone(), s.decl_index)).collect();
    order.sort_by_key(|(_, idx)| *idx);

    let mut status = LayoutStatus::Complete;
    let target = alloc.target;

    for (name, _) in order {
        let Some(sym) = scope.get_own_mut(&name) else { continue };
        if !matches!(sym.kind, SymbolKind::Variable | SymbolKind::Parameter) {
            continue;
        }
        let width = sym.ty.as_ref().map(|t| width_of(t, &target)).unwrap_or(Width::Unresolved);
        match width {
            Width::Known(w) => {
                let is_param = matches!(sym.kind, SymbolKind::Parameter);
                let offset = if is_param { alloc.alloc_param(w) } else { alloc.alloc_local(w) };
                sym.storage = Some(StorageInfo { offset, size_bytes: w });
            }
            Width::Unresolved => {
                status = LayoutStatus::Incomplete;
            }
        }
    }
    status
}

/// Tamaños de instancia y tablas de métodos, una vez asignadas las clases.
#[derive(Debug, Default)]
pub struct ClassSizes {
    sizes: HashMap<String, usize>,
    vtables: HashMap<String, Vec<String>>,
    incomplete: HashSet<String>,
}

impl ClassSizes {
    pub fn instance_size(&self, class: &str) -> Option<usize> {
        self.sizes.get(class).copied()
    }

    pub fn vtable(&self, class: &str) -> Option<&[String]> {
        self.vtables.get(class).map(Vec::as_slice)
    }

    /// `true` si la clase tuvo un campo `Unknown` o un padre que no se pudo
    /// resolver (inexistente, o que forma parte de un ciclo de herencia). En
    /// cualquiera de los dos casos la clase igual recibe un tamaño (con base
    /// 0 en el caso del padre) para que el resto del layout pueda seguir,
    /// pero la fase de TAC no debe confiar en él como si fuera definitivo.
    pub fn is_incomplete(&self, class: &str) -> bool {
        self.incomplete.contains(class)
    }
}

/// Orden en que hay que procesar `classes` para que el padre de cada clase
/// ya tenga su tamaño calculado: DFS por la cadena `parent`, con un ciclo
/// cortado en el punto donde se vuelve a visitar una clase que ya se estaba
/// visitando (no hay forma correcta de ordenar un ciclo; se prefiere no
/// colgarse a elegir "la" respuesta).
fn topological_class_order(classes: &[&mut Symbol]) -> Vec<usize> {
    let name_to_idx: HashMap<&str, usize> =
        classes.iter().enumerate().map(|(i, c)| (c.name.as_str(), i)).collect();
    let mut order = Vec::with_capacity(classes.len());
    let mut visited = vec![false; classes.len()];
    let mut visiting = vec![false; classes.len()];

    fn visit(
        idx: usize,
        classes: &[&mut Symbol],
        name_to_idx: &HashMap<&str, usize>,
        visited: &mut [bool],
        visiting: &mut [bool],
        order: &mut Vec<usize>,
    ) {
        if visited[idx] || visiting[idx] {
            return;
        }
        visiting[idx] = true;
        if let Some(parent) = &classes[idx].parent {
            if let Some(&pidx) = name_to_idx.get(parent.as_str()) {
                visit(pidx, classes, name_to_idx, visited, visiting, order);
            }
        }
        visiting[idx] = false;
        visited[idx] = true;
        order.push(idx);
    }

    for i in 0..classes.len() {
        visit(i, classes, &name_to_idx, &mut visited, &mut visiting, &mut order);
    }
    order
}

/// Calcula el tamaño de instancia y la tabla de métodos de cada clase en
/// `classes`, en el orden que haga falta para que un `Hijo` siempre empiece
/// justo donde termina su `Padre` — layout compatible por herencia, como
/// pide el plan. Escribe `Symbol.storage` en cada campo propio (`Variable`)
/// y `Symbol.storage_size` en la clase misma.
///
/// No valida nada semánticamente: un padre inexistente ya generó `S007`/
/// `S008` en el analizador; acá solo se decide qué hacer con el layout
/// cuando eso pasa (base 0, `is_incomplete` en `true`).
pub fn allocate_classes(classes: &mut [&mut Symbol], target: &TargetLayout) -> ClassSizes {
    let order = topological_class_order(classes);
    let mut result = ClassSizes::default();

    for idx in order {
        let class_name = classes[idx].name.clone();
        let parent_name = classes[idx].parent.clone();

        let (mut offset, inherited_vtable, parent_incomplete) = match &parent_name {
            Some(p) if result.sizes.contains_key(p) => {
                (result.sizes[p], result.vtables.get(p).cloned().unwrap_or_default(), result.incomplete.contains(p))
            }
            Some(_) => (0usize, Vec::new(), true),
            None => (0usize, Vec::new(), false),
        };

        let mut own_incomplete = parent_incomplete;
        let mut vtable = inherited_vtable;

        if let Some(members) = classes[idx].members.as_mut() {
            let mut member_order: Vec<usize> = (0..members.len()).collect();
            member_order.sort_by_key(|&i| members[i].decl_index);

            for i in member_order {
                let member = &mut members[i];
                match member.kind {
                    SymbolKind::Variable => {
                        let width = member.ty.as_ref().map(|t| width_of(t, target)).unwrap_or(Width::Unresolved);
                        match width {
                            Width::Known(w) => {
                                let align = natural_align(w, target.word);
                                offset = align_up(offset, align);
                                member.storage = Some(StorageInfo { offset: offset as isize, size_bytes: w });
                                offset += w;
                            }
                            Width::Unresolved => own_incomplete = true,
                        }
                    }
                    SymbolKind::Function | SymbolKind::Other(_) => {
                        if !vtable.iter().any(|m| m == &member.name) {
                            vtable.push(member.name.clone());
                        }
                        // Si ya estaba (override), reusa el índice heredado:
                        // no se toca `vtable`, la posición existente sigue
                        // siendo la misma.
                    }
                    _ => {}
                }
            }
        }

        let instance_size = align_up(offset, target.pointer.max(1));
        classes[idx].storage_size = Some(instance_size);
        result.sizes.insert(class_name.clone(), instance_size);
        result.vtables.insert(class_name.clone(), vtable);
        if own_incomplete {
            result.incomplete.insert(class_name);
        }
    }

    result
}

/// El resultado consolidado de la asignación de almacenamiento para todo un
/// programa — lo que una fase de TAC consulta. Lo llena `analyzer::analyze`
/// (marcos al cerrar cada función, el área estática y las clases al final) y
/// viaja en `AnalysisResult::layout`.
#[derive(Debug, Default)]
pub struct LayoutReport {
    pub classes: ClassSizes,
    /// `frame_size` de cada ámbito con marco propio, por `scope_id`
    /// (`scopes::Scope::id`) — incluye el área estática del Global bajo su
    /// propio id (0).
    pub frames: HashMap<usize, usize>,
    /// Ámbitos cuyo layout quedó incompleto por un símbolo `Unknown`.
    pub incomplete_scopes: HashSet<usize>,
    /// De qué función es cada marco de `frames` (por el mismo `scope_id`) y
    /// su nivel de anidamiento estático — lo que la fase de TAC necesita para
    /// armar el prólogo y el enlace de acceso. El área estática (id 0) no
    /// aparece: no es de ninguna función.
    pub functions: HashMap<usize, FrameOwner>,
}

/// La función dueña de un marco de activación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameOwner {
    pub name: String,
    /// Ver `Symbol::nesting_level`.
    pub level: usize,
}

impl LayoutReport {
    /// `true` si ningún marco ni ninguna clase quedó incompleto: la condición
    /// para que una fase de TAC pueda confiar en TODAS las direcciones.
    pub fn is_complete(&self) -> bool {
        self.incomplete_scopes.is_empty() && self.classes.incomplete.is_empty()
    }
}

/// Volcado legible del reporte — mismo espíritu que `scopes::ScopeCollector::dump`:
/// texto para depurar/CLI, no la forma que consumiría un consumidor
/// programático (para eso están los campos públicos de `LayoutReport`).
pub fn dump(report: &LayoutReport) -> String {
    let mut out = String::new();
    if report.frames.is_empty() && report.classes.sizes.is_empty() {
        return "(sin datos de almacenamiento)\n".to_string();
    }
    let mut frame_ids: Vec<&usize> = report.frames.keys().collect();
    frame_ids.sort();
    for id in frame_ids {
        let size = report.frames[id];
        let marker = if report.incomplete_scopes.contains(id) { " [incompleto]" } else { "" };
        let owner = match report.functions.get(id) {
            Some(f) => format!(" {} (nivel {})", f.name, f.level),
            None if *id == 0 => " estático".to_string(),
            None => String::new(),
        };
        out.push_str(&format!("marco #{id}{owner}: {size} bytes{marker}\n"));
    }
    let mut class_names: Vec<&String> = report.classes.sizes.keys().collect();
    class_names.sort();
    for name in class_names {
        let size = report.classes.sizes[name];
        let marker = if report.classes.is_incomplete(name) { " [incompleto]" } else { "" };
        out.push_str(&format!("clase {name}: {size} bytes{marker}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantico::scopes::ScopeKind;
    use crate::semantico::symbols::SymbolTable;

    #[test]
    fn anchos_de_los_tipos_primitivos_y_compuestos() {
        let target = TargetLayout::default();
        assert_eq!(width_of(&Type::Int, &target), Width::Known(4));
        assert_eq!(width_of(&Type::Float, &target), Width::Known(8));
        assert_eq!(width_of(&Type::Bool, &target), Width::Known(1));
        assert_eq!(width_of(&Type::Void, &target), Width::Known(0));
        assert_eq!(width_of(&Type::Str, &target), Width::Known(4));
        assert_eq!(width_of(&Type::Array(Box::new(Type::Int)), &target), Width::Known(4));
        assert_eq!(width_of(&Type::Map(Box::new(Type::Str), Box::new(Type::Int)), &target), Width::Known(4));
        assert_eq!(width_of(&Type::Set(Box::new(Type::Int)), &target), Width::Known(4));
        assert_eq!(width_of(&Type::Named("Punto".to_string()), &target), Width::Known(4));
        assert_eq!(width_of(&Type::Tuple(vec![Type::Int, Type::Int]), &target), Width::Known(8));
        assert_eq!(width_of(&Type::Unknown, &target), Width::Unresolved);
    }

    #[test]
    fn tupla_calcula_con_alineacion_natural_y_relleno() {
        let target = TargetLayout::default();
        // (bool, int): el bool ocupa 1 byte, pero el int que sigue necesita
        // alinearse a 4 -> 3 bytes de relleno en medio.
        let t = Type::Tuple(vec![Type::Bool, Type::Int]);
        assert_eq!(width_of(&t, &target), Width::Known(8));
    }

    #[test]
    fn tupla_con_un_miembro_unknown_es_unresolved() {
        let target = TargetLayout::default();
        let t = Type::Tuple(vec![Type::Int, Type::Unknown]);
        assert_eq!(width_of(&t, &target), Width::Unresolved);
    }

    fn declared_scope(decls: &[(&str, SymbolKind, Option<Type>)]) -> Scope {
        let mut table = SymbolTable::new();
        table.enter_scope(ScopeKind::Function, 1, 1);
        for (name, kind, ty) in decls {
            table.declare(name, kind.clone(), 1, 1).unwrap();
            if let Some(ty) = ty {
                table.lookup_mut(name).unwrap().ty = Some(ty.clone());
            }
        }
        table.exit_scope().unwrap()
    }

    #[test]
    fn parametro_offset_positivo_local_offset_negativo() {
        let target = TargetLayout::default();
        let mut scope = declared_scope(&[
            ("p", SymbolKind::Parameter, Some(Type::Int)),
            ("x", SymbolKind::Variable, Some(Type::Int)),
        ]);
        let mut alloc = FrameAllocator::new_function(&target);
        let status = allocate_scope(&mut scope, &mut alloc);
        assert_eq!(status, LayoutStatus::Complete);
        assert_eq!(
            scope.get_own("p").unwrap().storage,
            Some(StorageInfo { offset: 12, size_bytes: 4 })
        );
        assert_eq!(
            scope.get_own("x").unwrap().storage,
            Some(StorageInfo { offset: -12, size_bytes: 4 })
        );
    }

    #[test]
    fn ranura_de_marco_redondea_a_palabra() {
        let target = TargetLayout::default();
        let mut scope = declared_scope(&[("b", SymbolKind::Variable, Some(Type::Bool))]);
        let mut alloc = FrameAllocator::new_function(&target);
        allocate_scope(&mut scope, &mut alloc);
        // Bool pesa 1 byte, pero la ranura de marco redondea a WORD (4).
        assert_eq!(scope.get_own("b").unwrap().storage.as_ref().unwrap().size_bytes, 1);
        assert_eq!(alloc.size(), 8 /* área fija */ + 4 /* ranura redondeada */);
    }

    #[test]
    fn bloque_anidado_acumula_en_el_marco_de_la_funcion() {
        let target = TargetLayout::default();
        let mut table = SymbolTable::new();
        let mut alloc = FrameAllocator::new_function(&target);

        table.enter_scope(ScopeKind::Function, 1, 1);
        table.declare("a", SymbolKind::Variable, 1, 1).unwrap();
        table.lookup_mut("a").unwrap().ty = Some(Type::Int);

        table.enter_scope(ScopeKind::Block, 2, 1);
        table.declare("b", SymbolKind::Variable, 2, 1).unwrap();
        table.lookup_mut("b").unwrap().ty = Some(Type::Int);
        let mut inner = table.exit_scope().unwrap();
        allocate_scope(&mut inner, &mut alloc);

        let mut outer = table.exit_scope().unwrap();
        allocate_scope(&mut outer, &mut alloc);

        let b_off = inner.get_own("b").unwrap().storage.as_ref().unwrap().offset;
        let a_off = outer.get_own("a").unwrap().storage.as_ref().unwrap().offset;
        assert_ne!(a_off, b_off, "no deben solaparse dentro del mismo marco");
        assert_eq!(alloc.size(), 8 + 8, "dos int redondeados a WORD, más el área fija");
    }

    #[test]
    fn ty_ausente_o_unknown_no_recibe_offset_y_marca_incompleto() {
        let target = TargetLayout::default();
        let mut scope = declared_scope(&[("x", SymbolKind::Variable, None)]);
        let mut alloc = FrameAllocator::new_function(&target);
        let status = allocate_scope(&mut scope, &mut alloc);
        assert_eq!(status, LayoutStatus::Incomplete);
        assert!(scope.get_own("x").unwrap().storage.is_none());

        let mut scope2 = declared_scope(&[("y", SymbolKind::Variable, Some(Type::Unknown))]);
        let mut alloc2 = FrameAllocator::new_function(&target);
        assert_eq!(allocate_scope(&mut scope2, &mut alloc2), LayoutStatus::Incomplete);
        assert!(scope2.get_own("y").unwrap().storage.is_none());
    }

    #[test]
    fn area_estatica_del_global_crece_positiva_desde_cero() {
        let target = TargetLayout::default();
        let mut scope = declared_scope(&[("g", SymbolKind::Variable, Some(Type::Int))]);
        let mut alloc = FrameAllocator::new_static(&target);
        allocate_scope(&mut scope, &mut alloc);
        assert_eq!(scope.get_own("g").unwrap().storage.as_ref().unwrap().offset, 0);
        assert_eq!(alloc.size(), 4);
    }

    #[test]
    fn herencia_pone_los_campos_del_hijo_despues_de_los_del_padre() {
        let target = TargetLayout::default();
        let mut padre = Symbol {
            name: "Padre".to_string(),
            kind: SymbolKind::Class,
            members: Some(vec![Symbol {
                name: "x".to_string(),
                kind: SymbolKind::Variable,
                ty: Some(Type::Int),
                decl_index: 0,
                ..Symbol::default()
            }]),
            ..Symbol::default()
        };
        let mut hijo = Symbol {
            name: "Hijo".to_string(),
            kind: SymbolKind::Class,
            parent: Some("Padre".to_string()),
            members: Some(vec![Symbol {
                name: "y".to_string(),
                kind: SymbolKind::Variable,
                ty: Some(Type::Int),
                decl_index: 0,
                ..Symbol::default()
            }]),
            ..Symbol::default()
        };
        // Orden de entrada deliberadamente al revés: la función debe
        // resolverlo sola vía orden topológico, no confiar en el `Vec`.
        let mut classes: Vec<&mut Symbol> = vec![&mut hijo, &mut padre];
        let sizes = allocate_classes(&mut classes, &target);

        assert_eq!(sizes.instance_size("Padre"), Some(4));
        assert_eq!(sizes.instance_size("Hijo"), Some(8));
        assert!(!sizes.is_incomplete("Hijo"));

        let y_offset = hijo.members.as_ref().unwrap()[0].storage.as_ref().unwrap().offset;
        assert_eq!(y_offset, 4, "'y' debe empezar donde termina 'x' del padre");
    }

    #[test]
    fn padre_inexistente_marca_incompleto_y_usa_base_cero() {
        let target = TargetLayout::default();
        let mut huerfana = Symbol {
            name: "Huerfana".to_string(),
            kind: SymbolKind::Class,
            parent: Some("Fantasma".to_string()),
            members: Some(vec![Symbol {
                name: "z".to_string(),
                kind: SymbolKind::Variable,
                ty: Some(Type::Int),
                decl_index: 0,
                ..Symbol::default()
            }]),
            ..Symbol::default()
        };
        let mut classes: Vec<&mut Symbol> = vec![&mut huerfana];
        let sizes = allocate_classes(&mut classes, &target);
        assert!(sizes.is_incomplete("Huerfana"));
        assert_eq!(sizes.instance_size("Huerfana"), Some(4));
    }

    #[test]
    fn ciclo_de_herencia_no_causa_panic_ni_bucle_infinito() {
        let target = TargetLayout::default();
        let mut a = Symbol {
            name: "A".to_string(),
            kind: SymbolKind::Class,
            parent: Some("B".to_string()),
            members: Some(vec![]),
            ..Symbol::default()
        };
        let mut b = Symbol {
            name: "B".to_string(),
            kind: SymbolKind::Class,
            parent: Some("A".to_string()),
            members: Some(vec![]),
            ..Symbol::default()
        };
        let mut classes: Vec<&mut Symbol> = vec![&mut a, &mut b];
        let sizes = allocate_classes(&mut classes, &target);
        assert!(sizes.instance_size("A").is_some());
        assert!(sizes.instance_size("B").is_some());
    }

    #[test]
    fn metodo_override_reusa_el_indice_de_vtable_del_padre() {
        let target = TargetLayout::default();
        let mut padre = Symbol {
            name: "Padre".to_string(),
            kind: SymbolKind::Class,
            members: Some(vec![Symbol {
                name: "hablar".to_string(),
                kind: SymbolKind::Function,
                decl_index: 0,
                ..Symbol::default()
            }]),
            ..Symbol::default()
        };
        let mut hijo = Symbol {
            name: "Hijo".to_string(),
            kind: SymbolKind::Class,
            parent: Some("Padre".to_string()),
            members: Some(vec![
                Symbol {
                    name: "hablar".to_string(),
                    kind: SymbolKind::Function,
                    decl_index: 0,
                    ..Symbol::default()
                },
                Symbol {
                    name: "correr".to_string(),
                    kind: SymbolKind::Function,
                    decl_index: 1,
                    ..Symbol::default()
                },
            ]),
            ..Symbol::default()
        };
        let mut classes: Vec<&mut Symbol> = vec![&mut hijo, &mut padre];
        let sizes = allocate_classes(&mut classes, &target);
        let vt = sizes.vtable("Hijo").unwrap();
        assert_eq!(vt, &["hablar".to_string(), "correr".to_string()]);

        // Un método no ocupa espacio de instancia: el tamaño de Hijo es 0
        // (no declaró campos), heredado de un Padre también sin campos.
        assert_eq!(sizes.instance_size("Hijo"), Some(0));
    }

    #[test]
    fn dump_reporta_marcos_incompletos_y_clases() {
        let mut report = LayoutReport::default();
        report.frames.insert(0, 40);
        report.incomplete_scopes.insert(0);
        let text = dump(&report);
        assert!(text.contains("marco #0 estático: 40 bytes [incompleto]"), "{text}");
    }
}
