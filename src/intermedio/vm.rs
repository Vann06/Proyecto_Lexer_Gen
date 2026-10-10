//! Infraestructura base del intérprete de código intermedio TAC.
//!
//! Por ahora este módulo define los valores, errores, resultado, estado
//! mínimo de la máquina y el acceso a operandos. La ejecución de
//! instrucciones, llamadas, heap y manejadores de excepciones se agrega en
//! pasos posteriores de la Fase 3.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use super::tac::{Base, BinOp, Instr, Operand, TacProgram, UnOp};

/// Valor que puede almacenar o producir una instrucción TAC.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    /// Dirección de memoria. Su interpretación concreta se incorpora junto
    /// con el heap y los marcos de activación.
    Pointer(usize),
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
    /// `tN[offset]` necesita resolver el puntero de `tN`. Eso se habilitará
    /// al implementar heap y marcos completos.
    IndirectMemoryUnavailable { temp: usize, offset: isize },
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
            RuntimeError::IndirectMemoryUnavailable { temp, offset } => {
                write!(
                    f,
                    "no se puede acceder a t{temp}[{offset}] sin memoria indirecta"
                )
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

/// Máquina virtual encargada de ejecutar un programa TAC.
pub struct Vm<'a> {
    program: &'a TacProgram,
    /// Temporales de `main`. Cuando se incorporen llamadas, cada marco tendrá
    /// su propio conjunto de temporales.
    temporaries: HashMap<usize, Value>,
    /// Área estática indexada por offset en bytes, igual que `Base::Global`.
    globals: HashMap<isize, Value>,
    /// Ranuras del único marco actual, indexadas por offset en bytes. Esta
    /// interfaz se reemplazará por una pila de marcos al implementar llamadas.
    current_frame: HashMap<isize, Value>,
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
            temporaries: HashMap::new(),
            globals: HashMap::new(),
            current_frame: HashMap::new(),
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
        self.temporaries.get(&index)
    }

    pub fn global(&self, offset: isize) -> Option<&Value> {
        self.globals.get(&offset)
    }

    pub fn frame_value(&self, offset: isize) -> Option<&Value> {
        self.current_frame.get(&offset)
    }

    /// Ejecuta secuencialmente el cuerpo de `main`.
    ///
    /// En esta etapa no hay contador de programa ni flujo de control: una
    /// etiqueta, salto o llamada se reporta como instrucción no soportada.
    pub fn run(&mut self) -> Result<VmResult, RuntimeError> {
        self.temporaries.clear();
        self.globals.clear();
        self.current_frame.clear();
        self.output.clear();
        self.steps = 0;

        let body = self
            .program
            .main
            .as_ref()
            .map(|main| main.body.clone())
            .unwrap_or_default();

        for instruction in &body {
            self.execute(instruction)?;
        }

        Ok(VmResult {
            output: self.output.clone(),
            steps: self.steps,
        })
    }

    fn execute(&mut self, instruction: &Instr) -> Result<(), RuntimeError> {
        if self.steps >= self.max_steps {
            return Err(RuntimeError::StepLimitExceeded {
                limit: self.max_steps,
            });
        }
        self.steps += 1;

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
            other => Err(RuntimeError::UnsupportedInstruction(other.to_string())),
        }
    }

    /// Obtiene el valor de un operando.
    ///
    /// Los offsets se usan directamente como bytes, tal como los produce el
    /// generador. `Var.name` y `Mem.name` no participan en la dirección: son
    /// información de presentación. El `fp` especial usado por `Instr::Link`
    /// será tratado por esa instrucción cuando existan llamadas; este método
    /// solo lee ranuras `fp[offset]`.
    pub fn read_operand(&self, operand: &Operand) -> Result<Value, RuntimeError> {
        match operand {
            Operand::Int(value) => Ok(Value::Int(*value)),
            Operand::Float(value) => Ok(Value::Float(*value)),
            Operand::Bool(value) => Ok(Value::Bool(*value)),
            Operand::Str(value) => Ok(Value::Str(value.clone())),
            Operand::Null => Ok(Value::Null),
            Operand::Addr(name) => Ok(Value::Address(name.clone())),
            Operand::Temp(index) => self
                .temporaries
                .get(index)
                .cloned()
                .ok_or(RuntimeError::UndefinedTemporary { index: *index }),
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
                self.temporaries.insert(*index, value);
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
                .current_frame
                .get(&offset)
                .cloned()
                .ok_or(RuntimeError::UndefinedFrameSlot { offset }),
            Base::Temp(temp) => Err(RuntimeError::IndirectMemoryUnavailable {
                temp: *temp,
                offset,
            }),
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
                self.current_frame.insert(offset, value);
                Ok(())
            }
            Base::Temp(temp) => Err(RuntimeError::IndirectMemoryUnavailable {
                temp: *temp,
                offset,
            }),
        }
    }
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
    use crate::intermedio::tac::TacFunction;

    fn run_main(body: Vec<Instr>) -> Result<VmResult, RuntimeError> {
        let mut main = TacFunction::new("main", 0, 0);
        main.body = body;
        let program = TacProgram {
            main: Some(main),
            ..TacProgram::default()
        };
        Vm::new(&program).run()
    }

    #[test]
    fn value_representa_literales_punteros_y_direcciones_tac() {
        assert_eq!(Value::Int(7), Value::Int(7));
        assert_eq!(Value::Float(2.5), Value::Float(2.5));
        assert_eq!(Value::Bool(true), Value::Bool(true));
        assert_eq!(Value::Str("hola".into()), Value::Str("hola".into()));
        assert_eq!(Value::Pointer(12), Value::Pointer(12));
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
    fn memoria_con_base_temporal_espera_heap_o_marcos_completos() {
        let program = TacProgram::default();
        let vm = Vm::new(&program);
        let indirect = Operand::mem(Base::Temp(2), 4);

        assert_eq!(
            vm.read_operand(&indirect),
            Err(RuntimeError::IndirectMemoryUnavailable { temp: 2, offset: 4 })
        );
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
}
