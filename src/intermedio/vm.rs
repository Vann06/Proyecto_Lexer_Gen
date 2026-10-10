//! Infraestructura base del intérprete de código intermedio TAC.
//!
//! Por ahora este módulo define los valores, errores, resultado, estado
//! mínimo de la máquina y el acceso a operandos. La ejecución de
//! instrucciones, llamadas, heap y manejadores de excepciones se agrega en
//! pasos posteriores de la Fase 3.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use super::tac::{Base, Operand, TacProgram};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
