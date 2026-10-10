//! Infraestructura base del intérprete de código intermedio TAC.
//!
//! Por ahora este módulo define los valores, errores, resultado, estado
//! mínimo de la máquina, el acceso a operandos, el flujo de control y un
//! heap por bloques. Las llamadas y los manejadores de excepciones se agregan
//! en pasos posteriores de la Fase 3.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use crate::semantico::storage::TargetLayout;

use super::tac::{Base, BinOp, Instr, Operand, TacFunction, TacProgram, UnOp};

/// Valor que puede almacenar o producir una instrucción TAC.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    /// Identificador estable de un bloque del heap. Los desplazamientos en
    /// bytes se mantienen separados en `Load`, `Store` y `Operand::Mem`.
    Pointer(usize),
    /// Identificador de un marco activo. Se distingue de `Pointer` para que
    /// un enlace de acceso nunca pueda confundirse con un bloque del heap.
    FramePointer(usize),
    /// Dirección simbólica de un dato, por ejemplo `&vt_Perro`.
    /// Es necesaria para `Operand::Addr` y `Instr::AddrOf`.
    Address(String),
    Null,
}

impl Value {
    fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_) => "integer",
            Value::Float(_) => "float",
            Value::Bool(_) => "boolean",
            Value::Str(_) => "string",
            Value::Pointer(_) => "pointer",
            Value::FramePointer(_) => "frame pointer",
            Value::Address(_) => "address",
            Value::Null => "null",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(value) => write!(f, "{value}"),
            Value::Float(value) => write!(f, "{value}"),
            Value::Bool(value) => write!(f, "{value}"),
            Value::Str(value) => write!(f, "{value}"),
            Value::Pointer(value) => write!(f, "ptr({value})"),
            Value::FramePointer(value) => write!(f, "fp#{value}"),
            Value::Address(name) => write!(f, "&{name}"),
            Value::Null => write!(f, "null"),
        }
    }
}

/// Error producido al ejecutar TAC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// La ejecución alcanzó el límite configurado de instrucciones.
    StepLimitExceeded { limit: usize },
    /// Una instrucción todavía no forma parte del subconjunto ejecutable de
    /// la VM. Permite ampliar el intérprete de manera incremental.
    UnsupportedInstruction(String),
    /// Se intentó leer un temporal antes de asignarle un valor.
    UndefinedTemporary { index: usize },
    /// Se intentó leer una posición del área estática todavía no escrita.
    UndefinedGlobal { offset: isize },
    /// Se intentó leer una posición del marco actual todavía no escrita.
    UndefinedFrameSlot { offset: isize },
    /// El operando no representa un destino al que se pueda asignar.
    InvalidWriteTarget(String),
    /// Una función contiene dos etiquetas con el mismo nombre.
    DuplicateLabel { label: String },
    /// Un salto apunta a una etiqueta que no existe en la función actual.
    UnknownLabel { label: String },
    /// `alloc` requiere una cantidad de bytes entera y no negativa.
    InvalidAllocationSize { value: String },
    /// Una operación de memoria esperaba un puntero al heap.
    ExpectedPointer { found: &'static str },
    /// El desplazamiento de una operación de memoria debe ser entero.
    ExpectedIntegerOffset { found: &'static str },
    /// El identificador de bloque no pertenece al heap actual.
    InvalidPointer { pointer: usize },
    /// El desplazamiento en bytes queda fuera del bloque reservado.
    HeapOutOfBounds {
        pointer: usize,
        offset: isize,
        size: usize,
    },
    /// La dirección pertenece al bloque, pero todavía no fue escrita.
    UninitializedHeapCell { pointer: usize, offset: isize },
    /// El índice no pertenece al rango lógico de una lista.
    IndexOutOfBounds { index: i64, length: i64 },
    /// La cabecera de una lista no contiene una longitud entera válida.
    InvalidListLength { found: &'static str },
    /// `check_zero` encontró cero. Es un error separado para que la futura
    /// pila de manejadores pueda interceptarlo antes de la operación.
    ZeroCheckFailed,
    /// Un enlace de acceso no contiene la dirección de un marco activo.
    ExpectedFramePointer { found: &'static str },
    /// Un puntero de marco ya no identifica un marco activo.
    InvalidFramePointer { frame: usize },
    /// La etiqueta de función no existe en el programa.
    UnknownFunction { name: String },
    /// Una dirección simbólica no corresponde a una vtable del programa.
    UnknownDataAddress { name: String },
    /// Un acceso a vtable debe caer en una ranura de puntero completa.
    UnalignedVTableOffset { name: String, offset: isize },
    /// El slot pedido no existe en la vtable.
    VTableOutOfBounds {
        name: String,
        offset: isize,
        entries: usize,
    },
    /// `callv` requiere la dirección simbólica de una función.
    ExpectedCallableAddress { found: &'static str },
    /// `call f, n` no coincide con los `param` pendientes.
    PendingArgumentCount { function: String, nargs: usize, pending: usize },
    /// Una llamada que espera resultado terminó sin devolver un valor.
    MissingReturnValue { function: String },
    /// La operación recibió valores cuyos tipos no corresponden al operador.
    InvalidOperands {
        operation: &'static str,
        left: &'static str,
        right: Option<&'static str>,
    },
    DivisionByZero,
    ModuloByZero,
    ArithmeticOverflow { operation: &'static str },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::StepLimitExceeded { limit } => {
                write!(f, "se excedió el límite de {limit} pasos de la VM")
            }
            RuntimeError::UnsupportedInstruction(instruction) => {
                write!(
                    f,
                    "instrucción TAC todavía no soportada por la VM: {instruction}"
                )
            }
            RuntimeError::UndefinedTemporary { index } => {
                write!(f, "el temporal t{index} no tiene un valor")
            }
            RuntimeError::UndefinedGlobal { offset } => {
                write!(f, "la posición global G[{offset}] no tiene un valor")
            }
            RuntimeError::UndefinedFrameSlot { offset } => {
                write!(f, "la posición fp[{offset}] no tiene un valor")
            }
            RuntimeError::InvalidWriteTarget(operand) => {
                write!(f, "no se puede escribir en el operando {operand}")
            }
            RuntimeError::DuplicateLabel { label } => {
                write!(f, "la etiqueta {label} está definida más de una vez")
            }
            RuntimeError::UnknownLabel { label } => {
                write!(f, "la etiqueta {label} no existe en la función actual")
            }
            RuntimeError::InvalidAllocationSize { value } => {
                write!(f, "tamaño inválido para alloc: {value}")
            }
            RuntimeError::ExpectedPointer { found } => {
                write!(f, "se esperaba un puntero al heap, se obtuvo {found}")
            }
            RuntimeError::ExpectedIntegerOffset { found } => {
                write!(f, "se esperaba un desplazamiento entero, se obtuvo {found}")
            }
            RuntimeError::InvalidPointer { pointer } => {
                write!(f, "el puntero ptr({pointer}) no pertenece al heap")
            }
            RuntimeError::HeapOutOfBounds {
                pointer,
                offset,
                size,
            } => write!(
                f,
                "acceso fuera de rango: ptr({pointer})[{offset}] en un bloque de {size} bytes"
            ),
            RuntimeError::UninitializedHeapCell { pointer, offset } => write!(
                f,
                "la dirección ptr({pointer})[{offset}] no tiene un valor"
            ),
            RuntimeError::IndexOutOfBounds { index, length } => {
                write!(f, "índice {index} fuera de rango para una lista de longitud {length}")
            }
            RuntimeError::InvalidListLength { found } => {
                write!(f, "la cabecera de la lista no contiene una longitud válida: {found}")
            }
            RuntimeError::ZeroCheckFailed => {
                write!(f, "check_zero detectó un divisor igual a cero")
            }
            RuntimeError::ExpectedFramePointer { found } => {
                write!(f, "se esperaba un puntero de marco, se obtuvo {found}")
            }
            RuntimeError::InvalidFramePointer { frame } => {
                write!(f, "el puntero fp#{frame} no identifica un marco activo")
            }
            RuntimeError::UnknownFunction { name } => {
                write!(f, "la función {name} no existe en el programa TAC")
            }
            RuntimeError::UnknownDataAddress { name } => {
                write!(f, "la dirección &{name} no corresponde a datos del programa")
            }
            RuntimeError::UnalignedVTableOffset { name, offset } => write!(
                f,
                "el offset {offset} no está alineado a una entrada de la vtable {name}"
            ),
            RuntimeError::VTableOutOfBounds {
                name,
                offset,
                entries,
            } => write!(
                f,
                "el offset {offset} queda fuera de la vtable {name}, que tiene {entries} entrada(s)"
            ),
            RuntimeError::ExpectedCallableAddress { found } => {
                write!(f, "callv esperaba una dirección de función, obtuvo {found}")
            }
            RuntimeError::PendingArgumentCount {
                function,
                nargs,
                pending,
            } => write!(
                f,
                "call {function}, {nargs} encontró {pending} parámetro(s) pendiente(s)"
            ),
            RuntimeError::MissingReturnValue { function } => {
                write!(f, "la función {function} terminó sin devolver un valor")
            }
            RuntimeError::InvalidOperands {
                operation,
                left,
                right: Some(right),
            } => write!(
                f,
                "operandos incompatibles para {operation}: {left} y {right}"
            ),
            RuntimeError::InvalidOperands {
                operation,
                left,
                right: None,
            } => write!(f, "operando incompatible para {operation}: {left}"),
            RuntimeError::DivisionByZero => write!(f, "división por cero"),
            RuntimeError::ModuloByZero => write!(f, "módulo por cero"),
            RuntimeError::ArithmeticOverflow { operation } => {
                write!(f, "desbordamiento aritmético en {operation}")
            }
        }
    }
}

impl Error for RuntimeError {}

/// Resultado observable de una ejecución terminada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmResult {
    pub output: String,
    pub steps: usize,
}

/// Límite predeterminado para detener programas que no terminan.
pub const DEFAULT_MAX_STEPS: usize = 100_000;

/// Bloque reservado por `alloc`.
///
/// `cells` usa directamente desplazamientos en bytes. TAC no anota el ancho
/// de cada `Load`/`Store`, por lo que la VM valida que el byte inicial esté
/// dentro del bloque y conserva un `Value` completo en esa dirección.
#[derive(Debug, Default)]
struct HeapBlock {
    size_bytes: usize,
    cells: HashMap<isize, Value>,
}

/// Heap no móvil: un `Pointer` es el índice estable de un bloque.
#[derive(Debug, Default)]
struct Heap {
    blocks: Vec<HeapBlock>,
}

impl Heap {
    fn clear(&mut self) {
        self.blocks.clear();
    }

    fn alloc(&mut self, size_bytes: usize) -> usize {
        let pointer = self.blocks.len();
        self.blocks.push(HeapBlock {
            size_bytes,
            cells: HashMap::new(),
        });
        pointer
    }

    fn validate_address(&self, pointer: usize, offset: isize) -> Result<(), RuntimeError> {
        let block = self
            .blocks
            .get(pointer)
            .ok_or(RuntimeError::InvalidPointer { pointer })?;
        if offset < 0 || offset as usize >= block.size_bytes {
            return Err(RuntimeError::HeapOutOfBounds {
                pointer,
                offset,
                size: block.size_bytes,
            });
        }
        Ok(())
    }

    fn load(&self, pointer: usize, offset: isize) -> Result<Value, RuntimeError> {
        self.validate_address(pointer, offset)?;
        self.blocks[pointer]
            .cells
            .get(&offset)
            .cloned()
            .ok_or(RuntimeError::UninitializedHeapCell { pointer, offset })
    }

    fn store(
        &mut self,
        pointer: usize,
        offset: isize,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.validate_address(pointer, offset)?;
        self.blocks[pointer].cells.insert(offset, value);
        Ok(())
    }
}

/// Segmento inmutable de datos para las vtables de `TacProgram::data`.
#[derive(Debug, Default)]
struct VTables {
    entries: HashMap<String, Vec<String>>,
}

impl VTables {
    fn load_program(&mut self, program: &TacProgram) {
        self.entries.clear();
        for item in &program.data {
            self.entries
                .insert(item.name.clone(), item.entries.clone());
        }
    }

    fn load(&self, name: &str, offset: isize) -> Result<Value, RuntimeError> {
        let entries = self
            .entries
            .get(name)
            .ok_or_else(|| RuntimeError::UnknownDataAddress {
                name: name.to_owned(),
            })?;
        let width = TargetLayout::default().pointer as isize;
        if offset < 0 || offset % width != 0 {
            return Err(RuntimeError::UnalignedVTableOffset {
                name: name.to_owned(),
                offset,
            });
        }
        let slot = (offset / width) as usize;
        entries
            .get(slot)
            .cloned()
            .map(Value::Address)
            .ok_or_else(|| RuntimeError::VTableOutOfBounds {
                name: name.to_owned(),
                offset,
                entries: entries.len(),
            })
    }
}

/// Registro de activación estructurado.
///
/// `slots` conserva los offsets en bytes del layout MIPS. Los temporales se
/// guardan aparte porque `TacFunction::max_temps` ya describe su área física
/// y la VM no necesita materializar sus ranuras de 8 bytes para interpretarlos.
#[derive(Debug, Default)]
struct Frame {
    temporaries: HashMap<usize, Value>,
    slots: HashMap<isize, Value>,
    pending_params: Vec<Value>,
    pending_link: Option<usize>,
}

impl Frame {
    fn for_call(
        caller: Option<usize>,
        return_address: Option<usize>,
        access_link: Option<usize>,
        params: Vec<Value>,
    ) -> Self {
        let target = TargetLayout::default();
        let mut slots = HashMap::new();

        // El marco estructurado conserva el layout observable acordado.
        slots.insert(
            0,
            caller.map(Value::FramePointer).unwrap_or(Value::Null),
        );
        slots.insert(target.return_value, Value::Null);
        slots.insert(
            target.access_link,
            access_link
                .map(Value::FramePointer)
                .unwrap_or(Value::Null),
        );
        slots.insert(
            -(target.word as isize),
            return_address
                .map(|pc| Value::Int(pc as i64))
                .unwrap_or(Value::Null),
        );
        slots.insert(-(target.word as isize * 2), Value::Null);

        let mut offset = target.param_base;
        for value in params {
            let width = parameter_slot_width(&value, &target);
            slots.insert(offset, value);
            offset += width as isize;
        }

        Self {
            temporaries: HashMap::new(),
            slots,
            pending_params: Vec::new(),
            pending_link: None,
        }
    }
}

fn parameter_slot_width(value: &Value, target: &TargetLayout) -> usize {
    let width = match value {
        Value::Float(_) => target.float,
        Value::Bool(_) => target.bool_,
        Value::Int(_) => target.int,
        Value::Str(_)
        | Value::Pointer(_)
        | Value::FramePointer(_)
        | Value::Address(_)
        | Value::Null => target.pointer,
    };
    width.max(1).div_ceil(target.word) * target.word
}

/// Máquina virtual encargada de ejecutar un programa TAC.
pub struct Vm<'a> {
    program: &'a TacProgram,
    /// Área estática indexada por offset en bytes, igual que `Base::Global`.
    globals: HashMap<isize, Value>,
    /// Pila de registros de activación. El último es el `$fp` actual.
    frames: Vec<Frame>,
    heap: Heap,
    vtables: VTables,
    output: String,
    steps: usize,
    max_steps: usize,
}

impl<'a> Vm<'a> {
    /// Crea una VM con el límite de pasos predeterminado.
    pub fn new(program: &'a TacProgram) -> Self {
        Self::with_max_steps(program, DEFAULT_MAX_STEPS)
    }

    /// Crea una VM con un límite de pasos explícito.
    pub fn with_max_steps(program: &'a TacProgram, max_steps: usize) -> Self {
        Self {
            program,
            globals: HashMap::new(),
            // Mantener un frame vacío permite probar read/write de operandos
            // aun antes de llamar a `run`.
            frames: vec![Frame::default()],
            heap: Heap::default(),
            vtables: VTables::default(),
            output: String::new(),
            steps: 0,
            max_steps,
        }
    }

    pub fn program(&self) -> &'a TacProgram {
        self.program
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    pub fn steps(&self) -> usize {
        self.steps
    }

    pub fn max_steps(&self) -> usize {
        self.max_steps
    }

    pub fn temporary(&self, index: usize) -> Option<&Value> {
        self.frames.last()?.temporaries.get(&index)
    }

    pub fn global(&self, offset: isize) -> Option<&Value> {
        self.globals.get(&offset)
    }

    pub fn frame_value(&self, offset: isize) -> Option<&Value> {
        self.frames.last()?.slots.get(&offset)
    }

    /// Ejecuta el cuerpo de `main` con un contador de programa local.
    pub fn run(&mut self) -> Result<VmResult, RuntimeError> {
        self.globals.clear();
        self.frames.clear();
        self.heap.clear();
        self.vtables.load_program(self.program);
        self.output.clear();
        self.steps = 0;

        if let Some(main) = self.program.main.clone() {
            self.frames.push(Frame::for_call(None, None, None, Vec::new()));
            self.execute_function(&main)?;
        } else {
            self.frames.push(Frame::default());
        }

        Ok(VmResult {
            output: self.output.clone(),
            steps: self.steps,
        })
    }

    fn execute_function(
        &mut self,
        function: &TacFunction,
    ) -> Result<Option<Value>, RuntimeError> {
        let labels = build_label_map(function)?;
        let mut pc = 0usize;

        while pc < function.body.len() {
            self.take_step()?;
            match &function.body[pc] {
                Instr::Label(_) => pc += 1,
                Instr::Goto(label) => pc = label_target(&labels, label)?,
                Instr::IfTrue { cond, label } => {
                    if self.read_condition(cond, "if")? {
                        pc = label_target(&labels, label)?;
                    } else {
                        pc += 1;
                    }
                }
                Instr::IfFalse { cond, label } => {
                    if !self.read_condition(cond, "ifFalse")? {
                        pc = label_target(&labels, label)?;
                    } else {
                        pc += 1;
                    }
                }
                Instr::IfRel {
                    left,
                    op,
                    right,
                    label,
                } => {
                    let left = self.read_operand(left)?;
                    let right = self.read_operand(right)?;
                    let result = if op.is_relational() {
                        eval_binary(*op, left, right)?
                    } else {
                        return Err(RuntimeError::InvalidOperands {
                            operation: op.symbol(),
                            left: left.type_name(),
                            right: Some(right.type_name()),
                        });
                    };
                    if matches!(result, Value::Bool(true)) {
                        pc = label_target(&labels, label)?;
                    } else {
                        pc += 1;
                    }
                }
                Instr::Param(operand) => {
                    let value = self.read_operand(operand)?;
                    self.current_frame_mut().pending_params.push(value);
                    pc += 1;
                }
                Instr::Link(operand) => {
                    let value = self.read_operand(operand)?;
                    let frame = match value {
                        Value::FramePointer(frame) if frame < self.frames.len() => frame,
                        Value::FramePointer(frame) => {
                            return Err(RuntimeError::InvalidFramePointer { frame })
                        }
                        value => {
                            return Err(RuntimeError::ExpectedFramePointer {
                                found: value.type_name(),
                            })
                        }
                    };
                    self.current_frame_mut().pending_link = Some(frame);
                    pc += 1;
                }
                Instr::Call { dst, func, nargs } => {
                    self.invoke_call(func, *nargs, dst.as_ref(), pc + 1)?;
                    pc += 1;
                }
                Instr::CallV {
                    dst,
                    target,
                    nargs,
                } => {
                    let target = self.read_operand(target)?;
                    let function = match target {
                        Value::Address(function) => function,
                        value => {
                            return Err(RuntimeError::ExpectedCallableAddress {
                                found: value.type_name(),
                            })
                        }
                    };
                    self.invoke_call(&function, *nargs, dst.as_ref(), pc + 1)?;
                    pc += 1;
                }
                Instr::Return(value) => {
                    let value = value
                        .as_ref()
                        .map(|operand| self.read_operand(operand))
                        .transpose()?;
                    if let Some(value) = &value {
                        let return_offset = TargetLayout::default().return_value;
                        self.current_frame_mut()
                            .slots
                            .insert(return_offset, value.clone());
                    }
                    return Ok(value);
                }
                instruction => {
                    self.execute_non_control(instruction)?;
                    pc += 1;
                }
            }
        }

        // `endfunc` implica retorno para procedimientos sin `return`.
        Ok(None)
    }

    fn current_frame_id(&self) -> usize {
        self.frames.len() - 1
    }

    fn current_frame(&self) -> &Frame {
        self.frames
            .last()
            .expect("la VM mantiene al menos un marco activo")
    }

    fn current_frame_mut(&mut self) -> &mut Frame {
        self.frames
            .last_mut()
            .expect("la VM mantiene al menos un marco activo")
    }

    fn invoke_call(
        &mut self,
        function: &str,
        nargs: usize,
        dst: Option<&Operand>,
        return_address: usize,
    ) -> Result<(), RuntimeError> {
        let called = self
            .program
            .functions
            .iter()
            .find(|candidate| candidate.name == function)
            .cloned()
            .ok_or_else(|| RuntimeError::UnknownFunction {
                name: function.to_owned(),
            })?;
        let pending = self.current_frame().pending_params.len();
        if pending != nargs {
            return Err(RuntimeError::PendingArgumentCount {
                function: function.to_owned(),
                nargs,
                pending,
            });
        }
        let params = std::mem::take(&mut self.current_frame_mut().pending_params);
        let access_link = self.current_frame_mut().pending_link.take();
        let caller = self.current_frame_id();
        self.frames.push(Frame::for_call(
            Some(caller),
            Some(return_address),
            access_link,
            params,
        ));

        // La recursión del intérprete representa el enlace de control; la
        // pila `frames` conserva el `$fp` observable.
        let returned = self.execute_function(&called);
        self.frames.pop();
        let returned = returned?;

        if let Some(dst) = dst {
            let value = returned.ok_or_else(|| RuntimeError::MissingReturnValue {
                function: function.to_owned(),
            })?;
            self.write_operand(dst, value)?;
        }
        Ok(())
    }

    fn take_step(&mut self) -> Result<(), RuntimeError> {
        if self.steps >= self.max_steps {
            return Err(RuntimeError::StepLimitExceeded {
                limit: self.max_steps,
            });
        }
        self.steps += 1;
        Ok(())
    }

    fn read_condition(
        &self,
        operand: &Operand,
        operation: &'static str,
    ) -> Result<bool, RuntimeError> {
        let value = self.read_operand(operand)?;
        match value {
            Value::Bool(value) => Ok(value),
            value => Err(RuntimeError::InvalidOperands {
                operation,
                left: value.type_name(),
                right: None,
            }),
        }
    }

    fn execute_non_control(&mut self, instruction: &Instr) -> Result<(), RuntimeError> {
        match instruction {
            Instr::Copy { dst, src } => {
                let value = self.read_operand(src)?;
                self.write_operand(dst, value)
            }
            Instr::Unary { dst, op, src } => {
                let value = self.read_operand(src)?;
                let result = eval_unary(*op, value)?;
                self.write_operand(dst, result)
            }
            Instr::Binary {
                dst,
                op,
                left,
                right,
            } => {
                // Leer ambas fuentes antes de escribir es indispensable cuando
                // el generador recicla un temporal como destino.
                let left = self.read_operand(left)?;
                let right = self.read_operand(right)?;
                let result = eval_binary(*op, left, right)?;
                self.write_operand(dst, result)
            }
            Instr::Print(operand) => {
                let value = self.read_operand(operand)?;
                self.output.push_str(&value.to_string());
                self.output.push('\n');
                Ok(())
            }
            Instr::Alloc { dst, size } => {
                let size = self.read_operand(size)?;
                let size_bytes = match size {
                    Value::Int(size) if size >= 0 => usize::try_from(size).map_err(|_| {
                        RuntimeError::InvalidAllocationSize {
                            value: size.to_string(),
                        }
                    })?,
                    value => {
                        return Err(RuntimeError::InvalidAllocationSize {
                            value: value.to_string(),
                        })
                    }
                };
                let pointer = self.heap.alloc(size_bytes);
                self.write_operand(dst, Value::Pointer(pointer))
            }
            Instr::Load { dst, base, index } => {
                let (base, offset) = self.read_memory_address(base, index)?;
                let value = match base {
                    Value::Pointer(pointer) => self.heap.load(pointer, offset)?,
                    Value::FramePointer(frame) => self
                        .frames
                        .get(frame)
                        .ok_or(RuntimeError::InvalidFramePointer { frame })?
                        .slots
                        .get(&offset)
                        .cloned()
                        .ok_or(RuntimeError::UndefinedFrameSlot { offset })?,
                    Value::Address(name) => self.vtables.load(&name, offset)?,
                    value => {
                        return Err(RuntimeError::ExpectedPointer {
                            found: value.type_name(),
                        })
                    }
                };
                self.write_operand(dst, value)
            }
            Instr::Store { base, index, src } => {
                let (pointer, offset) = self.read_heap_address(base, index)?;
                let value = self.read_operand(src)?;
                self.heap.store(pointer, offset, value)
            }
            Instr::AddrOf { dst, name } => {
                if !self.vtables.entries.contains_key(name) {
                    return Err(RuntimeError::UnknownDataAddress { name: name.clone() });
                }
                self.write_operand(dst, Value::Address(name.clone()))
            }
            Instr::CheckBounds { array, index } => {
                let array = self.read_operand(array)?;
                let pointer = match array {
                    Value::Pointer(pointer) => pointer,
                    value => {
                        return Err(RuntimeError::ExpectedPointer {
                            found: value.type_name(),
                        })
                    }
                };
                let index = self.read_operand(index)?;
                let index = match index {
                    Value::Int(index) => index,
                    value => {
                        return Err(RuntimeError::InvalidOperands {
                            operation: "check_bounds",
                            left: value.type_name(),
                            right: None,
                        })
                    }
                };
                let length = self.heap.load(pointer, 0)?;
                let length = match length {
                    Value::Int(length) if length >= 0 => length,
                    value => {
                        return Err(RuntimeError::InvalidListLength {
                            found: value.type_name(),
                        })
                    }
                };
                if index < 0 || index >= length {
                    Err(RuntimeError::IndexOutOfBounds { index, length })
                } else {
                    Ok(())
                }
            }
            Instr::CheckZero(operand) => {
                let value = self.read_operand(operand)?;
                match value {
                    Value::Int(0) => Err(RuntimeError::ZeroCheckFailed),
                    Value::Float(value) if value == 0.0 => {
                        Err(RuntimeError::ZeroCheckFailed)
                    }
                    Value::Int(_) | Value::Float(_) => Ok(()),
                    value => Err(RuntimeError::InvalidOperands {
                        operation: "check_zero",
                        left: value.type_name(),
                        right: None,
                    }),
                }
            }
            other => Err(RuntimeError::UnsupportedInstruction(other.to_string())),
        }
    }

    fn read_heap_address(
        &self,
        base: &Operand,
        index: &Operand,
    ) -> Result<(usize, isize), RuntimeError> {
        let (base, offset) = self.read_memory_address(base, index)?;
        let pointer = match base {
            Value::Pointer(pointer) => pointer,
            value => {
                return Err(RuntimeError::ExpectedPointer {
                    found: value.type_name(),
                })
            }
        };
        Ok((pointer, offset))
    }

    fn read_memory_address(
        &self,
        base: &Operand,
        index: &Operand,
    ) -> Result<(Value, isize), RuntimeError> {
        let base = self.read_operand(base)?;
        let index = self.read_operand(index)?;
        let offset = match index {
            Value::Int(offset) => isize::try_from(offset).map_err(|_| {
                RuntimeError::ExpectedIntegerOffset {
                    found: "integer outside address range",
                }
            })?,
            value => {
                return Err(RuntimeError::ExpectedIntegerOffset {
                    found: value.type_name(),
                })
            }
        };
        Ok((base, offset))
    }

    /// Obtiene el valor de un operando.
    ///
    /// Los offsets se usan directamente como bytes, tal como los produce el
    /// generador. `Var.name` y `Mem.name` no participan en la dirección: son
    /// información de presentación. La variable sintética `fp` que emite
    /// `gen/func.rs` representa el puntero al marco actual; `fp[offset]`
    /// continúa representando una ranura de ese marco.
    pub fn read_operand(&self, operand: &Operand) -> Result<Value, RuntimeError> {
        match operand {
            Operand::Int(value) => Ok(Value::Int(*value)),
            Operand::Float(value) => Ok(Value::Float(*value)),
            Operand::Bool(value) => Ok(Value::Bool(*value)),
            Operand::Str(value) => Ok(Value::Str(value.clone())),
            Operand::Null => Ok(Value::Null),
            Operand::Addr(name) => Ok(Value::Address(name.clone())),
            Operand::Temp(index) => self
                .current_frame()
                .temporaries
                .get(index)
                .cloned()
                .ok_or(RuntimeError::UndefinedTemporary { index: *index }),
            Operand::Var {
                name,
                base: Base::Fp,
                offset: 0,
            } if name == "fp" => Ok(Value::FramePointer(self.current_frame_id())),
            Operand::Var { base, offset, .. } | Operand::Mem { base, offset, .. } => {
                self.read_location(base, *offset)
            }
        }
    }

    /// Escribe un valor en un temporal o en una dirección directa de `G` o
    /// del marco actual. Los literales y las direcciones simbólicas no son
    /// destinos válidos.
    pub fn write_operand(
        &mut self,
        operand: &Operand,
        value: Value,
    ) -> Result<(), RuntimeError> {
        match operand {
            Operand::Temp(index) => {
                self.current_frame_mut().temporaries.insert(*index, value);
                Ok(())
            }
            Operand::Var { base, offset, .. } | Operand::Mem { base, offset, .. } => {
                self.write_location(base, *offset, value)
            }
            Operand::Int(_)
            | Operand::Float(_)
            | Operand::Bool(_)
            | Operand::Str(_)
            | Operand::Null
            | Operand::Addr(_) => Err(RuntimeError::InvalidWriteTarget(operand.to_string())),
        }
    }

    fn read_location(&self, base: &Base, offset: isize) -> Result<Value, RuntimeError> {
        match base {
            Base::Global => self
                .globals
                .get(&offset)
                .cloned()
                .ok_or(RuntimeError::UndefinedGlobal { offset }),
            Base::Fp => self
                .current_frame()
                .slots
                .get(&offset)
                .cloned()
                .ok_or(RuntimeError::UndefinedFrameSlot { offset }),
            Base::Temp(temp) => {
                let value = self
                    .current_frame()
                    .temporaries
                    .get(temp)
                    .ok_or(RuntimeError::UndefinedTemporary { index: *temp })?;
                match value {
                    Value::Pointer(pointer) => self.heap.load(*pointer, offset),
                    Value::FramePointer(frame) => self
                        .frames
                        .get(*frame)
                        .ok_or(RuntimeError::InvalidFramePointer { frame: *frame })?
                        .slots
                        .get(&offset)
                        .cloned()
                        .ok_or(RuntimeError::UndefinedFrameSlot { offset }),
                    Value::Address(name) => self.vtables.load(name, offset),
                    value => Err(RuntimeError::ExpectedPointer {
                        found: value.type_name(),
                    }),
                }
            }
        }
    }

    fn write_location(
        &mut self,
        base: &Base,
        offset: isize,
        value: Value,
    ) -> Result<(), RuntimeError> {
        match base {
            Base::Global => {
                self.globals.insert(offset, value);
                Ok(())
            }
            Base::Fp => {
                self.current_frame_mut().slots.insert(offset, value);
                Ok(())
            }
            Base::Temp(temp) => {
                let pointer = match self
                    .current_frame()
                    .temporaries
                    .get(temp)
                    .ok_or(RuntimeError::UndefinedTemporary { index: *temp })?
                {
                    Value::Pointer(pointer) => EitherPointer::Heap(*pointer),
                    Value::FramePointer(frame) => EitherPointer::Frame(*frame),
                    value => {
                        return Err(RuntimeError::ExpectedPointer {
                            found: value.type_name(),
                        })
                    }
                };
                match pointer {
                    EitherPointer::Heap(pointer) => self.heap.store(pointer, offset, value),
                    EitherPointer::Frame(frame) => {
                        let frame = self
                            .frames
                            .get_mut(frame)
                            .ok_or(RuntimeError::InvalidFramePointer { frame })?;
                        frame.slots.insert(offset, value);
                        Ok(())
                    }
                }
            }
        }
    }
}

enum EitherPointer {
    Heap(usize),
    Frame(usize),
}

fn build_label_map(function: &TacFunction) -> Result<HashMap<String, usize>, RuntimeError> {
    let mut labels = HashMap::new();
    for (index, instruction) in function.body.iter().enumerate() {
        if let Instr::Label(label) = instruction {
            if labels.insert(label.clone(), index).is_some() {
                return Err(RuntimeError::DuplicateLabel {
                    label: label.clone(),
                });
            }
        }
    }
    Ok(labels)
}

fn label_target(
    labels: &HashMap<String, usize>,
    label: &str,
) -> Result<usize, RuntimeError> {
    labels
        .get(label)
        .copied()
        .ok_or_else(|| RuntimeError::UnknownLabel {
            label: label.to_owned(),
        })
}

fn invalid_unary(op: UnOp, value: &Value) -> RuntimeError {
    RuntimeError::InvalidOperands {
        operation: op.symbol(),
        left: value.type_name(),
        right: None,
    }
}

fn invalid_binary(op: BinOp, left: &Value, right: &Value) -> RuntimeError {
    RuntimeError::InvalidOperands {
        operation: op.symbol(),
        left: left.type_name(),
        right: Some(right.type_name()),
    }
}

fn eval_unary(op: UnOp, value: Value) -> Result<Value, RuntimeError> {
    match (op, value) {
        (UnOp::Neg, Value::Int(value)) => value
            .checked_neg()
            .map(Value::Int)
            .ok_or(RuntimeError::ArithmeticOverflow {
                operation: UnOp::Neg.symbol(),
            }),
        (UnOp::NegF, Value::Float(value)) => Ok(Value::Float(-value)),
        (UnOp::Not, Value::Bool(value)) => Ok(Value::Bool(!value)),
        (UnOp::IntToFloat, Value::Int(value)) => Ok(Value::Float(value as f64)),
        (op, value) => Err(invalid_unary(op, &value)),
    }
}

fn comparable_equality(left: &Value, right: &Value) -> Option<bool> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => Some(a == b),
        (Value::Float(a), Value::Float(b)) => Some(a == b),
        (Value::Bool(a), Value::Bool(b)) => Some(a == b),
        (Value::Str(a), Value::Str(b)) => Some(a == b),
        (Value::Pointer(a), Value::Pointer(b)) => Some(a == b),
        (Value::FramePointer(a), Value::FramePointer(b)) => Some(a == b),
        (Value::Address(a), Value::Address(b)) => Some(a == b),
        (Value::Null, Value::Null) => Some(true),
        _ => None,
    }
}

fn eval_binary(op: BinOp, left: Value, right: Value) -> Result<Value, RuntimeError> {
    if matches!(op, BinOp::Eq | BinOp::Ne) {
        return comparable_equality(&left, &right)
            .map(|equal| Value::Bool(if op == BinOp::Eq { equal } else { !equal }))
            .ok_or_else(|| invalid_binary(op, &left, &right));
    }

    let invalid = || invalid_binary(op, &left, &right);
    match (op, &left, &right) {
        (BinOp::Add, Value::Int(a), Value::Int(b)) => a
            .checked_add(*b)
            .map(Value::Int)
            .ok_or(RuntimeError::ArithmeticOverflow { operation: "+" }),
        (BinOp::Sub, Value::Int(a), Value::Int(b)) => a
            .checked_sub(*b)
            .map(Value::Int)
            .ok_or(RuntimeError::ArithmeticOverflow { operation: "-" }),
        (BinOp::Mul, Value::Int(a), Value::Int(b)) => a
            .checked_mul(*b)
            .map(Value::Int)
            .ok_or(RuntimeError::ArithmeticOverflow { operation: "*" }),
        (BinOp::Div, Value::Int(_), Value::Int(0)) => Err(RuntimeError::DivisionByZero),
        (BinOp::Div, Value::Int(a), Value::Int(b)) => a
            .checked_div(*b)
            .map(Value::Int)
            .ok_or(RuntimeError::ArithmeticOverflow { operation: "/" }),
        (BinOp::Mod, Value::Int(_), Value::Int(0)) => Err(RuntimeError::ModuloByZero),
        (BinOp::Mod, Value::Int(a), Value::Int(b)) => a
            .checked_rem(*b)
            .map(Value::Int)
            .ok_or(RuntimeError::ArithmeticOverflow { operation: "%" }),
        (BinOp::AddF, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (BinOp::SubF, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        (BinOp::MulF, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        (BinOp::DivF, Value::Float(_), Value::Float(b)) if *b == 0.0 => {
            Err(RuntimeError::DivisionByZero)
        }
        (BinOp::DivF, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
        (BinOp::Concat, Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{a}{b}"))),
        (BinOp::Lt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
        (BinOp::Le, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
        (BinOp::Gt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
        (BinOp::Ge, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
        (BinOp::Lt, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a < b)),
        (BinOp::Le, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a <= b)),
        (BinOp::Gt, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a > b)),
        (BinOp::Ge, Value::Float(a), Value::Float(b)) => Ok(Value::Bool(a >= b)),
        _ => Err(invalid()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intermedio::tac::DataItem;

    fn program_with_main(body: Vec<Instr>) -> TacProgram {
        let mut main = TacFunction::new("main", 0, 0);
        main.body = body;
        TacProgram {
            main: Some(main),
            ..TacProgram::default()
        }
    }

    fn program_with_functions(body: Vec<Instr>, functions: Vec<TacFunction>) -> TacProgram {
        let mut program = program_with_main(body);
        program.functions = functions;
        program
    }

    fn function(
        name: &str,
        level: usize,
        frame_size: usize,
        body: Vec<Instr>,
    ) -> TacFunction {
        let mut function = TacFunction::new(name, level, frame_size);
        function.body = body;
        function
    }

    fn run_main(body: Vec<Instr>) -> Result<VmResult, RuntimeError> {
        let program = program_with_main(body);
        Vm::new(&program).run()
    }

    #[test]
    fn value_representa_literales_punteros_y_direcciones_tac() {
        assert_eq!(Value::Int(7), Value::Int(7));
        assert_eq!(Value::Float(2.5), Value::Float(2.5));
        assert_eq!(Value::Bool(true), Value::Bool(true));
        assert_eq!(Value::Str("hola".into()), Value::Str("hola".into()));
        assert_eq!(Value::Pointer(12), Value::Pointer(12));
        assert_eq!(Value::FramePointer(2), Value::FramePointer(2));
        assert_eq!(
            Value::Address("vt_Animal".into()),
            Value::Address("vt_Animal".into())
        );
        assert_eq!(Value::Null, Value::Null);
    }

    #[test]
    fn una_vm_nueva_conserva_el_programa_y_empieza_vacia() {
        let program = TacProgram::default();
        let vm = Vm::with_max_steps(&program, 25);

        assert!(std::ptr::eq(vm.program(), &program));
        assert_eq!(vm.output(), "");
        assert_eq!(vm.steps(), 0);
        assert_eq!(vm.max_steps(), 25);
        assert_eq!(vm.temporary(0), None);
        assert_eq!(vm.global(0), None);
        assert_eq!(vm.frame_value(0), None);
    }

    #[test]
    fn runtime_error_tiene_un_mensaje_legible() {
        let error = RuntimeError::StepLimitExceeded { limit: 10 };
        assert_eq!(error.to_string(), "se excedió el límite de 10 pasos de la VM");
    }

    #[test]
    fn lee_literales_enteros_y_booleanos() {
        let program = TacProgram::default();
        let vm = Vm::new(&program);

        assert_eq!(vm.read_operand(&Operand::Int(5)), Ok(Value::Int(5)));
        assert_eq!(
            vm.read_operand(&Operand::Bool(true)),
            Ok(Value::Bool(true))
        );
    }

    #[test]
    fn escribe_y_vuelve_a_leer_un_temporal() {
        let program = TacProgram::default();
        let mut vm = Vm::new(&program);
        let t0 = Operand::Temp(0);

        vm.write_operand(&t0, Value::Int(5)).unwrap();

        assert_eq!(vm.read_operand(&t0), Ok(Value::Int(5)));
    }

    #[test]
    fn escribe_y_lee_una_global_por_su_offset() {
        let program = TacProgram {
            static_size: 8,
            ..TacProgram::default()
        };
        let mut vm = Vm::new(&program);
        let global = Operand::global("x", 4);

        vm.write_operand(&global, Value::Int(9)).unwrap();

        assert_eq!(vm.read_operand(&global), Ok(Value::Int(9)));
        assert_eq!(
            vm.read_operand(&Operand::mem(Base::Global, 4)),
            Ok(Value::Int(9))
        );
    }

    #[test]
    fn una_variable_local_usa_el_offset_del_frame_actual() {
        let program = TacProgram::default();
        let mut vm = Vm::new(&program);
        let local = Operand::local("total", -12);

        vm.write_operand(&local, Value::Int(3)).unwrap();

        assert_eq!(vm.read_operand(&local), Ok(Value::Int(3)));
        assert_eq!(vm.frame_value(-12), Some(&Value::Int(3)));
    }

    #[test]
    fn ejecuta_copy_y_print_sobre_un_programa_tac_real() {
        let result = run_main(vec![
            Instr::Copy {
                dst: Operand::Temp(0),
                src: Operand::Int(5),
            },
            Instr::Print(Operand::Temp(0)),
        ])
        .unwrap();

        assert_eq!(result.output, "5\n");
        assert_eq!(result.steps, 2);
    }

    #[test]
    fn suma_dos_temporales_y_los_imprime() {
        let result = run_main(vec![
            Instr::Copy {
                dst: Operand::Temp(0),
                src: Operand::Int(2),
            },
            Instr::Copy {
                dst: Operand::Temp(1),
                src: Operand::Int(3),
            },
            Instr::Binary {
                dst: Operand::Temp(2),
                op: BinOp::Add,
                left: Operand::Temp(0),
                right: Operand::Temp(1),
            },
            Instr::Print(Operand::Temp(2)),
        ])
        .unwrap();

        assert_eq!(result.output, "5\n");
    }

    #[test]
    fn multiplicacion_entera_imprime_seis() {
        let result = run_main(vec![
            Instr::Copy {
                dst: Operand::Temp(0),
                src: Operand::Int(2),
            },
            Instr::Copy {
                dst: Operand::Temp(1),
                src: Operand::Int(3),
            },
            Instr::Binary {
                dst: Operand::Temp(2),
                op: BinOp::Mul,
                left: Operand::Temp(0),
                right: Operand::Temp(1),
            },
            Instr::Print(Operand::Temp(2)),
        ])
        .unwrap();

        assert_eq!(result.output, "6\n");
    }

    #[test]
    fn ejecuta_todos_los_operadores_enteros() {
        let mut body = vec![
            Instr::Copy {
                dst: Operand::Temp(0),
                src: Operand::Int(7),
            },
            Instr::Copy {
                dst: Operand::Temp(1),
                src: Operand::Int(2),
            },
        ];
        for (temp, op) in [BinOp::Add, BinOp::Sub, BinOp::Mul, BinOp::Div, BinOp::Mod]
            .into_iter()
            .enumerate()
        {
            let dst = Operand::Temp(temp + 2);
            body.push(Instr::Binary {
                dst: dst.clone(),
                op,
                left: Operand::Temp(0),
                right: Operand::Temp(1),
            });
            body.push(Instr::Print(dst));
        }

        let result = run_main(body).unwrap();

        assert_eq!(result.output, "9\n5\n14\n3\n1\n");
    }

    #[test]
    fn ejecuta_todos_los_operadores_float() {
        let mut body = vec![
            Instr::Copy {
                dst: Operand::Temp(0),
                src: Operand::Float(8.0),
            },
            Instr::Copy {
                dst: Operand::Temp(1),
                src: Operand::Float(2.0),
            },
        ];
        for (temp, op) in [BinOp::AddF, BinOp::SubF, BinOp::MulF, BinOp::DivF]
            .into_iter()
            .enumerate()
        {
            let dst = Operand::Temp(temp + 2);
            body.push(Instr::Binary {
                dst: dst.clone(),
                op,
                left: Operand::Temp(0),
                right: Operand::Temp(1),
            });
            body.push(Instr::Print(dst));
        }

        let result = run_main(body).unwrap();

        assert_eq!(result.output, "10\n6\n16\n4\n");
    }

    #[test]
    fn ejecuta_minus_minusf_not_y_conversion_a_float() {
        let result = run_main(vec![
            Instr::Unary {
                dst: Operand::Temp(0),
                op: UnOp::Neg,
                src: Operand::Int(5),
            },
            Instr::Print(Operand::Temp(0)),
            Instr::Unary {
                dst: Operand::Temp(1),
                op: UnOp::NegF,
                src: Operand::Float(2.5),
            },
            Instr::Print(Operand::Temp(1)),
            Instr::Unary {
                dst: Operand::Temp(2),
                op: UnOp::Not,
                src: Operand::Bool(true),
            },
            Instr::Print(Operand::Temp(2)),
            Instr::Unary {
                dst: Operand::Temp(3),
                op: UnOp::IntToFloat,
                src: Operand::Int(3),
            },
            Instr::Print(Operand::Temp(3)),
        ])
        .unwrap();

        assert_eq!(result.output, "-5\n-2.5\nfalse\n3\n");
    }

    #[test]
    fn concatena_strings() {
        let result = run_main(vec![
            Instr::Binary {
                dst: Operand::Temp(0),
                op: BinOp::Concat,
                left: Operand::Str("hola ".into()),
                right: Operand::Str("mundo".into()),
            },
            Instr::Print(Operand::Temp(0)),
        ])
        .unwrap();

        assert_eq!(result.output, "hola mundo\n");
    }

    #[test]
    fn ejecuta_todas_las_comparaciones() {
        let mut body = Vec::new();
        for (temp, op) in [
            BinOp::Eq,
            BinOp::Ne,
            BinOp::Lt,
            BinOp::Le,
            BinOp::Gt,
            BinOp::Ge,
        ]
        .into_iter()
        .enumerate()
        {
            let dst = Operand::Temp(temp);
            body.push(Instr::Binary {
                dst: dst.clone(),
                op,
                left: Operand::Int(2),
                right: Operand::Int(3),
            });
            body.push(Instr::Print(dst));
        }

        let result = run_main(body).unwrap();

        assert_eq!(result.output, "false\ntrue\ntrue\ntrue\nfalse\nfalse\n");
    }

    #[test]
    fn division_y_modulo_por_cero_son_errores() {
        let div = run_main(vec![Instr::Binary {
            dst: Operand::Temp(0),
            op: BinOp::Div,
            left: Operand::Int(5),
            right: Operand::Int(0),
        }]);
        let modulo = run_main(vec![Instr::Binary {
            dst: Operand::Temp(0),
            op: BinOp::Mod,
            left: Operand::Int(5),
            right: Operand::Int(0),
        }]);

        assert_eq!(div, Err(RuntimeError::DivisionByZero));
        assert_eq!(modulo, Err(RuntimeError::ModuloByZero));
    }

    #[test]
    fn valores_incompatibles_producen_un_error() {
        let result = run_main(vec![Instr::Binary {
            dst: Operand::Temp(0),
            op: BinOp::Add,
            left: Operand::Str("x".into()),
            right: Operand::Int(1),
        }]);

        assert_eq!(
            result,
            Err(RuntimeError::InvalidOperands {
                operation: "+",
                left: "string",
                right: Some("integer"),
            })
        );
    }

    #[test]
    fn goto_salta_instrucciones_hasta_la_etiqueta() {
        let result = run_main(vec![
            Instr::Goto("fin".into()),
            Instr::Print(Operand::Int(99)),
            Instr::Label("fin".into()),
            Instr::Print(Operand::Int(1)),
        ])
        .unwrap();

        assert_eq!(result.output, "1\n");
    }

    #[test]
    fn if_true_salta_cuando_la_condicion_es_verdadera() {
        let result = run_main(vec![
            Instr::IfTrue {
                cond: Operand::Bool(true),
                label: "verdadero".into(),
            },
            Instr::Print(Operand::Int(0)),
            Instr::Goto("fin".into()),
            Instr::Label("verdadero".into()),
            Instr::Print(Operand::Int(1)),
            Instr::Label("fin".into()),
        ])
        .unwrap();

        assert_eq!(result.output, "1\n");
    }

    #[test]
    fn if_false_salta_cuando_la_condicion_es_falsa() {
        let result = run_main(vec![
            Instr::IfFalse {
                cond: Operand::Bool(false),
                label: "falso".into(),
            },
            Instr::Print(Operand::Int(1)),
            Instr::Goto("fin".into()),
            Instr::Label("falso".into()),
            Instr::Print(Operand::Int(0)),
            Instr::Label("fin".into()),
        ])
        .unwrap();

        assert_eq!(result.output, "0\n");
    }

    #[test]
    fn if_rel_evalua_la_comparacion_y_salta() {
        let result = run_main(vec![
            Instr::IfRel {
                left: Operand::Int(2),
                op: BinOp::Lt,
                right: Operand::Int(3),
                label: "menor".into(),
            },
            Instr::Print(Operand::Bool(false)),
            Instr::Goto("fin".into()),
            Instr::Label("menor".into()),
            Instr::Print(Operand::Bool(true)),
            Instr::Label("fin".into()),
        ])
        .unwrap();

        assert_eq!(result.output, "true\n");
    }

    #[test]
    fn ejecuta_un_while_representado_manualmente_en_tac() {
        let result = run_main(vec![
            Instr::Copy {
                dst: Operand::Temp(0),
                src: Operand::Int(0),
            },
            Instr::Label("while".into()),
            Instr::IfRel {
                left: Operand::Temp(0),
                op: BinOp::Ge,
                right: Operand::Int(3),
                label: "fin".into(),
            },
            Instr::Print(Operand::Temp(0)),
            Instr::Binary {
                dst: Operand::Temp(0),
                op: BinOp::Add,
                left: Operand::Temp(0),
                right: Operand::Int(1),
            },
            Instr::Goto("while".into()),
            Instr::Label("fin".into()),
        ])
        .unwrap();

        assert_eq!(result.output, "0\n1\n2\n");
    }

    #[test]
    fn un_loop_infinito_se_detiene_en_el_limite_de_pasos() {
        let program = program_with_main(vec![
            Instr::Label("loop".into()),
            Instr::Goto("loop".into()),
        ]);
        let mut vm = Vm::with_max_steps(&program, 7);

        assert_eq!(
            vm.run(),
            Err(RuntimeError::StepLimitExceeded { limit: 7 })
        );
        assert_eq!(vm.steps(), 7);
    }

    #[test]
    fn alloc_reserva_un_bloque_y_devuelve_un_puntero() {
        let program = program_with_main(vec![Instr::Alloc {
            dst: Operand::Temp(0),
            size: Operand::Int(8),
        }]);
        let mut vm = Vm::new(&program);

        vm.run().unwrap();

        assert_eq!(vm.temporary(0), Some(&Value::Pointer(0)));
        assert_eq!(vm.heap.blocks.len(), 1);
        assert_eq!(vm.heap.blocks[0].size_bytes, 8);
    }

    #[test]
    fn store_y_load_comparten_un_offset_en_bytes() {
        let result = run_main(vec![
            Instr::Alloc {
                dst: Operand::Temp(0),
                size: Operand::Int(8),
            },
            Instr::Store {
                base: Operand::Temp(0),
                index: Operand::Int(4),
                src: Operand::Int(42),
            },
            Instr::Load {
                dst: Operand::Temp(1),
                base: Operand::Temp(0),
                index: Operand::Int(4),
            },
            Instr::Print(Operand::Temp(1)),
        ])
        .unwrap();

        assert_eq!(result.output, "42\n");
    }

    #[test]
    fn dos_bloques_del_heap_son_independientes() {
        let result = run_main(vec![
            Instr::Alloc {
                dst: Operand::Temp(0),
                size: Operand::Int(4),
            },
            Instr::Alloc {
                dst: Operand::Temp(1),
                size: Operand::Int(4),
            },
            Instr::Store {
                base: Operand::Temp(0),
                index: Operand::Int(0),
                src: Operand::Int(10),
            },
            Instr::Store {
                base: Operand::Temp(1),
                index: Operand::Int(0),
                src: Operand::Int(20),
            },
            Instr::Load {
                dst: Operand::Temp(2),
                base: Operand::Temp(0),
                index: Operand::Int(0),
            },
            Instr::Load {
                dst: Operand::Temp(3),
                base: Operand::Temp(1),
                index: Operand::Int(0),
            },
            Instr::Print(Operand::Temp(2)),
            Instr::Print(Operand::Temp(3)),
        ])
        .unwrap();

        assert_eq!(result.output, "10\n20\n");
    }

    #[test]
    fn acceso_fuera_del_bloque_del_heap_es_un_error() {
        let result = run_main(vec![
            Instr::Alloc {
                dst: Operand::Temp(0),
                size: Operand::Int(4),
            },
            Instr::Store {
                base: Operand::Temp(0),
                index: Operand::Int(4),
                src: Operand::Int(1),
            },
        ]);

        assert_eq!(
            result,
            Err(RuntimeError::HeapOutOfBounds {
                pointer: 0,
                offset: 4,
                size: 4,
            })
        );
    }

    #[test]
    fn operand_mem_con_base_temp_accede_al_heap() {
        let result = run_main(vec![
            Instr::Alloc {
                dst: Operand::Temp(0),
                size: Operand::Int(8),
            },
            Instr::Copy {
                dst: Operand::mem(Base::Temp(0), 4),
                src: Operand::Int(9),
            },
            Instr::Print(Operand::mem(Base::Temp(0), 4)),
        ])
        .unwrap();

        assert_eq!(result.output, "9\n");
    }

    #[test]
    fn check_bounds_acepta_un_indice_valido() {
        let result = run_main(vec![
            Instr::Alloc {
                dst: Operand::Temp(0),
                size: Operand::Int(8),
            },
            Instr::Store {
                base: Operand::Temp(0),
                index: Operand::Int(0),
                src: Operand::Int(1),
            },
            Instr::CheckBounds {
                array: Operand::Temp(0),
                index: Operand::Int(0),
            },
            Instr::Print(Operand::Bool(true)),
        ])
        .unwrap();

        assert_eq!(result.output, "true\n");
    }

    #[test]
    fn check_bounds_rechaza_un_indice_negativo() {
        let result = run_main(vec![
            Instr::Alloc {
                dst: Operand::Temp(0),
                size: Operand::Int(12),
            },
            Instr::Store {
                base: Operand::Temp(0),
                index: Operand::Int(0),
                src: Operand::Int(2),
            },
            Instr::CheckBounds {
                array: Operand::Temp(0),
                index: Operand::Int(-1),
            },
        ]);

        assert_eq!(
            result,
            Err(RuntimeError::IndexOutOfBounds {
                index: -1,
                length: 2,
            })
        );
    }

    #[test]
    fn check_bounds_rechaza_un_indice_igual_a_la_longitud() {
        let result = run_main(vec![
            Instr::Alloc {
                dst: Operand::Temp(0),
                size: Operand::Int(12),
            },
            Instr::Store {
                base: Operand::Temp(0),
                index: Operand::Int(0),
                src: Operand::Int(2),
            },
            Instr::CheckBounds {
                array: Operand::Temp(0),
                index: Operand::Int(2),
            },
        ]);

        assert_eq!(
            result,
            Err(RuntimeError::IndexOutOfBounds {
                index: 2,
                length: 2,
            })
        );
    }

    #[test]
    fn check_zero_detecta_cero_entero_y_float() {
        assert_eq!(
            run_main(vec![Instr::CheckZero(Operand::Int(0))]),
            Err(RuntimeError::ZeroCheckFailed)
        );
        assert_eq!(
            run_main(vec![Instr::CheckZero(Operand::Float(-0.0))]),
            Err(RuntimeError::ZeroCheckFailed)
        );
        assert!(run_main(vec![
            Instr::CheckZero(Operand::Int(1)),
            Instr::CheckZero(Operand::Float(0.5)),
        ])
        .is_ok());
    }

    #[test]
    fn division_por_cero_conserva_su_error_especifico() {
        let result = run_main(vec![Instr::Binary {
            dst: Operand::Temp(0),
            op: BinOp::Div,
            left: Operand::Int(1),
            right: Operand::Int(0),
        }]);

        assert_eq!(result, Err(RuntimeError::DivisionByZero));
    }

    #[test]
    fn modulo_por_cero_conserva_su_error_especifico() {
        let result = run_main(vec![Instr::Binary {
            dst: Operand::Temp(0),
            op: BinOp::Mod,
            left: Operand::Int(1),
            right: Operand::Int(0),
        }]);

        assert_eq!(result, Err(RuntimeError::ModuloByZero));
    }

    #[test]
    fn llama_un_procedimiento_sin_parametros() {
        let procedure = function(
            "saludar",
            1,
            8,
            vec![Instr::Print(Operand::Str("hola".into()))],
        );
        let program = program_with_functions(
            vec![Instr::Call {
                dst: None,
                func: "saludar".into(),
                nargs: 0,
            }],
            vec![procedure],
        );

        assert_eq!(Vm::new(&program).run().unwrap().output, "hola\n");
    }

    #[test]
    fn pasa_dos_parametros_en_fp_mas_16_y_fp_mas_20() {
        let sum = function(
            "suma",
            1,
            16,
            vec![
                Instr::Binary {
                    dst: Operand::Temp(0),
                    op: BinOp::Add,
                    left: Operand::local("a", 16),
                    right: Operand::local("b", 20),
                },
                Instr::Return(Some(Operand::Temp(0))),
            ],
        );
        let program = program_with_functions(
            vec![
                Instr::Param(Operand::Int(2)),
                Instr::Param(Operand::Int(3)),
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "suma".into(),
                    nargs: 2,
                },
                Instr::Print(Operand::Temp(0)),
            ],
            vec![sum],
        );

        assert_eq!(Vm::new(&program).run().unwrap().output, "5\n");
    }

    #[test]
    fn propaga_un_retorno_entero_al_destino_del_call() {
        let answer = function(
            "respuesta",
            1,
            8,
            vec![Instr::Return(Some(Operand::Int(42)))],
        );
        let program = program_with_functions(
            vec![
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "respuesta".into(),
                    nargs: 0,
                },
                Instr::Print(Operand::Temp(0)),
            ],
            vec![answer],
        );

        assert_eq!(Vm::new(&program).run().unwrap().output, "42\n");
    }

    #[test]
    fn main_puede_llamar_una_funcion_y_continuar() {
        let one = function(
            "uno",
            1,
            8,
            vec![Instr::Return(Some(Operand::Int(1)))],
        );
        let program = program_with_functions(
            vec![
                Instr::Print(Operand::Int(0)),
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "uno".into(),
                    nargs: 0,
                },
                Instr::Print(Operand::Temp(0)),
                Instr::Print(Operand::Int(2)),
            ],
            vec![one],
        );

        assert_eq!(Vm::new(&program).run().unwrap().output, "0\n1\n2\n");
    }

    #[test]
    fn ejecuta_factorial_recursivo() {
        let factorial = function(
            "factorial",
            1,
            16,
            vec![
                Instr::IfRel {
                    left: Operand::local("n", 16),
                    op: BinOp::Le,
                    right: Operand::Int(1),
                    label: "base".into(),
                },
                Instr::Binary {
                    dst: Operand::Temp(0),
                    op: BinOp::Sub,
                    left: Operand::local("n", 16),
                    right: Operand::Int(1),
                },
                Instr::Param(Operand::Temp(0)),
                Instr::Call {
                    dst: Some(Operand::Temp(1)),
                    func: "factorial".into(),
                    nargs: 1,
                },
                Instr::Binary {
                    dst: Operand::Temp(2),
                    op: BinOp::Mul,
                    left: Operand::local("n", 16),
                    right: Operand::Temp(1),
                },
                Instr::Return(Some(Operand::Temp(2))),
                Instr::Label("base".into()),
                Instr::Return(Some(Operand::Int(1))),
            ],
        );
        let program = program_with_functions(
            vec![
                Instr::Param(Operand::Int(5)),
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "factorial".into(),
                    nargs: 1,
                },
                Instr::Print(Operand::Temp(0)),
            ],
            vec![factorial],
        );

        assert_eq!(Vm::new(&program).run().unwrap().output, "120\n");
    }

    #[test]
    fn llamadas_consecutivas_no_comparten_temporales_del_callee() {
        let identity = function(
            "identidad",
            1,
            16,
            vec![
                Instr::Copy {
                    dst: Operand::Temp(0),
                    src: Operand::local("n", 16),
                },
                Instr::Return(Some(Operand::Temp(0))),
            ],
        );
        let program = program_with_functions(
            vec![
                Instr::Param(Operand::Int(7)),
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "identidad".into(),
                    nargs: 1,
                },
                Instr::Print(Operand::Temp(0)),
                Instr::Param(Operand::Int(9)),
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "identidad".into(),
                    nargs: 1,
                },
                Instr::Print(Operand::Temp(0)),
            ],
            vec![identity],
        );

        assert_eq!(Vm::new(&program).run().unwrap().output, "7\n9\n");
    }

    #[test]
    fn una_funcion_anidada_lee_el_frame_de_su_entorno() {
        let outer = function(
            "externa",
            1,
            16,
            vec![
                Instr::Copy {
                    dst: Operand::local("capturada", -12),
                    src: Operand::Int(40),
                },
                Instr::Link(Operand::Var {
                    name: "fp".into(),
                    base: Base::Fp,
                    offset: 0,
                }),
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "externa.interna".into(),
                    nargs: 0,
                },
                Instr::Return(Some(Operand::Temp(0))),
            ],
        );
        let inner = function(
            "externa.interna",
            2,
            16,
            vec![
                Instr::Copy {
                    dst: Operand::Temp(0),
                    src: Operand::mem(Base::Fp, 12),
                },
                Instr::Return(Some(Operand::Mem {
                    base: Base::Temp(0),
                    offset: -12,
                    name: Some("capturada".into()),
                })),
            ],
        );
        let program = program_with_functions(
            vec![
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "externa".into(),
                    nargs: 0,
                },
                Instr::Print(Operand::Temp(0)),
            ],
            vec![outer, inner],
        );

        assert_eq!(Vm::new(&program).run().unwrap().output, "40\n");
    }

    #[test]
    fn contador_sumar_lee_y_escribe_total_y_paso_del_frame_exterior() {
        let contador = function(
            "contador",
            1,
            20,
            vec![
                Instr::Copy {
                    dst: Operand::local("total", -12),
                    src: Operand::Int(0),
                },
                Instr::Link(Operand::Var {
                    name: "fp".into(),
                    base: Base::Fp,
                    offset: 0,
                }),
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "contador.sumar".into(),
                    nargs: 0,
                },
                Instr::Return(Some(Operand::Temp(0))),
            ],
        );
        let sumar = function(
            "contador.sumar",
            2,
            24,
            vec![
                Instr::Copy {
                    dst: Operand::Temp(0),
                    src: Operand::mem(Base::Fp, 12),
                },
                Instr::Copy {
                    dst: Operand::Temp(1),
                    src: Operand::mem(Base::Fp, 12),
                },
                Instr::Binary {
                    dst: Operand::Temp(2),
                    op: BinOp::Add,
                    left: Operand::Mem {
                        base: Base::Temp(0),
                        offset: -12,
                        name: Some("total".into()),
                    },
                    right: Operand::Mem {
                        base: Base::Temp(1),
                        offset: 16,
                        name: Some("paso".into()),
                    },
                },
                Instr::Copy {
                    dst: Operand::Mem {
                        base: Base::Temp(0),
                        offset: -12,
                        name: Some("total".into()),
                    },
                    src: Operand::Temp(2),
                },
                Instr::Return(Some(Operand::Mem {
                    base: Base::Temp(0),
                    offset: -12,
                    name: Some("total".into()),
                })),
            ],
        );
        let program = program_with_functions(
            vec![
                Instr::Param(Operand::Int(5)),
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "contador".into(),
                    nargs: 1,
                },
                Instr::Print(Operand::Temp(0)),
            ],
            vec![contador, sumar],
        );

        assert_eq!(Vm::new(&program).run().unwrap().output, "5\n");
    }

    #[test]
    fn dos_enlaces_de_acceso_llegan_al_frame_del_abuelo_lexico() {
        let outer = function(
            "outer",
            1,
            16,
            vec![
                Instr::Copy {
                    dst: Operand::local("x", -12),
                    src: Operand::Int(7),
                },
                Instr::Link(Operand::Var {
                    name: "fp".into(),
                    base: Base::Fp,
                    offset: 0,
                }),
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "outer.middle".into(),
                    nargs: 0,
                },
                Instr::Return(Some(Operand::Temp(0))),
            ],
        );
        let middle = function(
            "outer.middle",
            2,
            16,
            vec![
                Instr::Link(Operand::Var {
                    name: "fp".into(),
                    base: Base::Fp,
                    offset: 0,
                }),
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "outer.middle.inner".into(),
                    nargs: 0,
                },
                Instr::Return(Some(Operand::Temp(0))),
            ],
        );
        let inner = function(
            "outer.middle.inner",
            3,
            16,
            vec![
                Instr::Copy {
                    dst: Operand::Temp(0),
                    src: Operand::mem(Base::Fp, 12),
                },
                Instr::Copy {
                    dst: Operand::Temp(0),
                    src: Operand::mem(Base::Temp(0), 12),
                },
                Instr::Return(Some(Operand::Mem {
                    base: Base::Temp(0),
                    offset: -12,
                    name: Some("x".into()),
                })),
            ],
        );
        let program = program_with_functions(
            vec![
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "outer".into(),
                    nargs: 0,
                },
                Instr::Print(Operand::Temp(0)),
            ],
            vec![outer, middle, inner],
        );

        assert_eq!(Vm::new(&program).run().unwrap().output, "7\n");
    }

    #[test]
    fn recursion_anidada_conserva_access_link_distinto_del_control_link() {
        let outer = function(
            "outer",
            1,
            16,
            vec![
                Instr::Copy {
                    dst: Operand::local("total", -12),
                    src: Operand::Int(0),
                },
                Instr::Param(Operand::Int(3)),
                Instr::Link(Operand::Var {
                    name: "fp".into(),
                    base: Base::Fp,
                    offset: 0,
                }),
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "outer.contar".into(),
                    nargs: 1,
                },
                Instr::Return(Some(Operand::Temp(0))),
            ],
        );
        let nested = function(
            "outer.contar",
            2,
            24,
            vec![
                Instr::IfRel {
                    left: Operand::local("n", 16),
                    op: BinOp::Le,
                    right: Operand::Int(0),
                    label: "fin".into(),
                },
                Instr::Copy {
                    dst: Operand::Temp(0),
                    src: Operand::mem(Base::Fp, 12),
                },
                Instr::Binary {
                    dst: Operand::Temp(1),
                    op: BinOp::Add,
                    left: Operand::Mem {
                        base: Base::Temp(0),
                        offset: -12,
                        name: Some("total".into()),
                    },
                    right: Operand::Int(1),
                },
                Instr::Copy {
                    dst: Operand::Mem {
                        base: Base::Temp(0),
                        offset: -12,
                        name: Some("total".into()),
                    },
                    src: Operand::Temp(1),
                },
                Instr::Binary {
                    dst: Operand::Temp(1),
                    op: BinOp::Sub,
                    left: Operand::local("n", 16),
                    right: Operand::Int(1),
                },
                Instr::Param(Operand::Temp(1)),
                Instr::Link(Operand::Temp(0)),
                Instr::Call {
                    dst: Some(Operand::Temp(1)),
                    func: "outer.contar".into(),
                    nargs: 1,
                },
                Instr::Return(Some(Operand::Temp(1))),
                Instr::Label("fin".into()),
                Instr::Copy {
                    dst: Operand::Temp(0),
                    src: Operand::mem(Base::Fp, 12),
                },
                Instr::Return(Some(Operand::Mem {
                    base: Base::Temp(0),
                    offset: -12,
                    name: Some("total".into()),
                })),
            ],
        );
        let program = program_with_functions(
            vec![
                Instr::Call {
                    dst: Some(Operand::Temp(0)),
                    func: "outer".into(),
                    nargs: 0,
                },
                Instr::Print(Operand::Temp(0)),
            ],
            vec![outer, nested],
        );

        assert_eq!(Vm::new(&program).run().unwrap().output, "3\n");
    }

    #[test]
    fn addrof_y_load_representan_una_vtable_inmutable() {
        let mut program = program_with_main(vec![
            Instr::AddrOf {
                dst: Operand::Temp(0),
                name: "vt_Simple".into(),
            },
            Instr::Load {
                dst: Operand::Temp(1),
                base: Operand::Temp(0),
                index: Operand::Int(0),
            },
        ]);
        program.data.push(DataItem {
            name: "vt_Simple".into(),
            entries: vec!["Simple.metodo".into()],
        });
        let mut vm = Vm::new(&program);

        vm.run().unwrap();

        assert_eq!(vm.temporary(0), Some(&Value::Address("vt_Simple".into())));
        assert_eq!(
            vm.temporary(1),
            Some(&Value::Address("Simple.metodo".into()))
        );
    }

    #[test]
    fn callv_ejecuta_un_metodo_y_recibe_this_como_primer_parametro() {
        let value_method = function(
            "Caja.valor",
            1,
            16,
            vec![
                Instr::Copy {
                    dst: Operand::Temp(0),
                    src: Operand::local("this", 16),
                },
                Instr::Return(Some(Operand::Mem {
                    base: Base::Temp(0),
                    offset: 4,
                    name: Some("valor".into()),
                })),
            ],
        );
        let mut program = program_with_functions(
            vec![
                Instr::Alloc {
                    dst: Operand::Temp(0),
                    size: Operand::Int(8),
                },
                Instr::Store {
                    base: Operand::Temp(0),
                    index: Operand::Int(0),
                    src: Operand::Addr("vt_Caja".into()),
                },
                Instr::Store {
                    base: Operand::Temp(0),
                    index: Operand::Int(4),
                    src: Operand::Int(17),
                },
                Instr::Param(Operand::Temp(0)),
                Instr::Load {
                    dst: Operand::Temp(1),
                    base: Operand::Temp(0),
                    index: Operand::Int(0),
                },
                Instr::Load {
                    dst: Operand::Temp(1),
                    base: Operand::Temp(1),
                    index: Operand::Int(0),
                },
                Instr::CallV {
                    dst: Some(Operand::Temp(2)),
                    target: Operand::Temp(1),
                    nargs: 1,
                },
                Instr::Print(Operand::Temp(2)),
            ],
            vec![value_method],
        );
        program.data.push(DataItem {
            name: "vt_Caja".into(),
            entries: vec!["Caja.valor".into()],
        });

        assert_eq!(Vm::new(&program).run().unwrap().output, "17\n");
    }

    #[test]
    fn una_vtable_hija_puede_reusar_un_metodo_heredado() {
        let inherited = function(
            "Animal.nombre",
            1,
            8,
            vec![Instr::Return(Some(Operand::Str("animal".into())))],
        );
        let mut program = program_with_functions(
            vec![
                Instr::Alloc {
                    dst: Operand::Temp(0),
                    size: Operand::Int(4),
                },
                Instr::Store {
                    base: Operand::Temp(0),
                    index: Operand::Int(0),
                    src: Operand::Addr("vt_Perro".into()),
                },
                Instr::Param(Operand::Temp(0)),
                Instr::Load {
                    dst: Operand::Temp(1),
                    base: Operand::Temp(0),
                    index: Operand::Int(0),
                },
                Instr::Load {
                    dst: Operand::Temp(1),
                    base: Operand::Temp(1),
                    index: Operand::Int(0),
                },
                Instr::CallV {
                    dst: Some(Operand::Temp(2)),
                    target: Operand::Temp(1),
                    nargs: 1,
                },
                Instr::Print(Operand::Temp(2)),
            ],
            vec![inherited],
        );
        program.data.push(DataItem {
            name: "vt_Perro".into(),
            entries: vec!["Animal.nombre".into()],
        });

        assert_eq!(Vm::new(&program).run().unwrap().output, "animal\n");
    }

    #[test]
    fn referencia_de_padre_sobre_objeto_hijo_despacha_al_override() {
        let animal = function(
            "Animal.hablar",
            1,
            8,
            vec![Instr::Return(Some(Operand::Str("...".into())))],
        );
        let dog = function(
            "Perro.hablar",
            1,
            8,
            vec![Instr::Return(Some(Operand::Str("guau".into())))],
        );
        // El código usa el slot 0 definido por el tipo estático Animal, pero
        // el objeto contiene vt_Perro y esa misma ranura apunta al override.
        let mut program = program_with_functions(
            vec![
                Instr::Alloc {
                    dst: Operand::Temp(0),
                    size: Operand::Int(4),
                },
                Instr::Store {
                    base: Operand::Temp(0),
                    index: Operand::Int(0),
                    src: Operand::Addr("vt_Perro".into()),
                },
                Instr::Param(Operand::Temp(0)),
                Instr::Load {
                    dst: Operand::Temp(1),
                    base: Operand::Temp(0),
                    index: Operand::Int(0),
                },
                Instr::Load {
                    dst: Operand::Temp(1),
                    base: Operand::Temp(1),
                    index: Operand::Int(0),
                },
                Instr::CallV {
                    dst: Some(Operand::Temp(2)),
                    target: Operand::Temp(1),
                    nargs: 1,
                },
                Instr::Print(Operand::Temp(2)),
            ],
            vec![animal, dog],
        );
        program.data.push(DataItem {
            name: "vt_Animal".into(),
            entries: vec!["Animal.hablar".into()],
        });
        program.data.push(DataItem {
            name: "vt_Perro".into(),
            entries: vec!["Perro.hablar".into()],
        });

        assert_eq!(Vm::new(&program).run().unwrap().output, "guau\n");
    }
}
