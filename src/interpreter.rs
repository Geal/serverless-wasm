use parity_wasm;
use parity_wasm::elements::{External, FunctionType, Internal, Type, ValueType};
use std::collections::VecDeque;
use wasmi::{self, Module};
use wasmi::{BlockFrameType, Externals, FuncInstance, FuncRef, FunctionContext, Interpreter, RunResult, RuntimeValue, Trap, TrapKind};
use std::marker;
use std::rc::Rc;
use std::cell::RefCell;

pub const DEFAULT_VALUE_STACK_LIMIT: usize = 16384;
pub const DEFAULT_FRAME_STACK_LIMIT: usize = 16384;

pub trait HostBuilder<'a, S> {
  fn build(s: &'a mut S) -> Self;
}

pub trait Host {
  type State;

  fn build(s: Rc<RefCell<Self::State>>) -> Self;
}

pub struct WasmInstance<S, E: Externals + Host<State = S>> {
  pub state: Rc<RefCell<S>>,
  pub stack: VecDeque<FunctionContext>,
  _marker: marker::PhantomData<E>,
}

impl<S, E: Externals + Host<State = S>> WasmInstance<S, E> {
  pub fn new(state: Rc<RefCell<S>>, func_ref: &FuncRef, args: &[RuntimeValue]) -> WasmInstance<S, E> {
    let stack = create_stack(&func_ref, args);

    WasmInstance {
      state: state,
      stack,
      _marker: marker::PhantomData,
    }
  }

  pub fn resume(&mut self) -> Result<Option<RuntimeValue>, Trap> {
    let mut host = E::build(self.state.clone());
    let mut interpreter = Interpreter::new(&mut host);

    println!("WasmInstance::resume: stack\n{:?}", self.stack);
    my_run_interpreter_loop(&mut interpreter, &mut self.stack)
  }

  pub fn add_function_result(&mut self, return_value: RuntimeValue) {
    self.stack.back_mut().map(|function_context| {
      function_context.value_stack_mut().push(return_value).expect("should have pushed the return value");
      println!("adding return value to {:?} initialized: {}",
        function_context.function, function_context.is_initialized);
    });
    println!("added function result {:?}, stack len:{}", return_value, self.stack.len());
  }
}

pub fn create_stack(func: &FuncRef, args: &[RuntimeValue]) -> VecDeque<FunctionContext> {
  let context = FunctionContext::new(
    func.clone(),
    DEFAULT_VALUE_STACK_LIMIT,
    DEFAULT_FRAME_STACK_LIMIT,
    func.signature(),
    args.into_iter().cloned().collect(),
  );

  let mut function_stack = VecDeque::new();
  function_stack.push_back(context);

  function_stack
}

pub fn my_run_interpreter_loop<E>(
  interpreter: &mut Interpreter<E>,
  function_stack: &mut VecDeque<FunctionContext>,
) -> Result<Option<RuntimeValue>, Trap>
where
  E: Externals,
{
  loop {
    let mut function_context = function_stack
      .pop_back()
      .expect("on loop entry - not empty; on loop continue - checking for emptiness; qed");
    let function_ref = function_context.function.clone();
    let function_body = function_ref
      .body()
      .expect("Host functions checked in function_return below; Internal functions always have a body; qed");
    if !function_context.is_initialized() {
      let return_type = function_context.return_type;
      function_context.initialize(&function_body.locals);
      function_context
        .push_frame(&function_body.labels, BlockFrameType::Function, return_type)
        .map_err(Trap::new)?;
    }

    let function_return = interpreter
      .do_run_function(
        &mut function_context,
        function_body.opcodes.elements(),
        &function_body.labels,
      )
      .map_err(Trap::new)?;

    match function_return {
      RunResult::Return(return_value) => match function_stack.back_mut() {
        Some(caller_context) => if let Some(return_value) = return_value {
          caller_context
            .value_stack_mut()
            .push(return_value)
            .map_err(Trap::new)?;
        },
        None => return Ok(return_value),
      },
      RunResult::NestedCall(nested_func) => {
        //println!("calling nested func, stack len={}", function_stack.len());
        match FuncInstance::invoke_context(&nested_func, &mut function_context, interpreter.externals) {
          Err(t) => {
            if let TrapKind::Host(_) = t.kind() {
              //function_context.value_stack_mut().push(RuntimeValue::I32(42)).expect("should have pushed the return value");
              function_stack.push_back(function_context);
              println!("got host trapkind");
              return Err(t);
            } else {
              println!("resume got error: {:?}", t);
              return Err(t);
            }
          },
          Ok(None) => {
            function_stack.push_back(function_context);
            //println!("got ok(none) stack len={}", function_stack.len());
          }
          Ok(Some(nested_context)) => {
            function_stack.push_back(function_context);
            function_stack.push_back(nested_context);
            //println!("got ok(some(nested_context)) stack len={}", function_stack.len());
          }
        }
      }
    }
  }
}

pub fn load_module(file: &str, func_name: &str) -> Module {
  let module = parity_wasm::deserialize_file(file).expect("File to be deserialized");

  // Extracts call arguments from command-line arguments
  let _args = {
    // Export section has an entry with a func_name with an index inside a module
    let export_section = module.export_section().expect("No export section found");
    // It's a section with function declarations (which are references to the type section entries)
    let function_section = module
      .function_section()
      .expect("No function section found");
    // Type section stores function types which are referenced by function_section entries
    let type_section = module.type_section().expect("No type section found");

    // Given function name used to find export section entry which contains
    // an `internal` field which points to the index in the function index space
    let found_entry = export_section
      .entries()
      .iter()
      .find(|entry| func_name == entry.field())
      .expect(&format!("No export with name {} found", func_name));

    // Function index in the function index space (internally-defined + imported)
    let function_index: usize = match found_entry.internal() {
      &Internal::Function(index) => index as usize,
      _ => panic!("Founded export is not a function"),
    };

    // We need to count import section entries (functions only!) to subtract it from function_index
    // and obtain the index within the function section
    let import_section_len: usize = match module.import_section() {
      Some(import) => import
        .entries()
        .iter()
        .map(|entry| {
          //println!("importing entry {:?}", entry);
          entry
        })
        .filter(|entry| match entry.external() {
          &External::Function(_) => true,
          _ => false,
        })
        .count(),
      None => 0,
    };

    // Calculates a function index within module's function section
    let function_index_in_section = function_index - import_section_len;

    // Getting a type reference from a function section entry
    let func_type_ref: usize = function_section.entries()[function_index_in_section].type_ref() as usize;

    // Use the reference to get an actual function type
    let function_type: &FunctionType = match &type_section.types()[func_type_ref] {
      &Type::Function(ref func_type) => func_type,
    };

    // Parses arguments and constructs runtime values in correspondence of their types
    function_type
      .params()
      .iter()
      .enumerate()
      .map(|(_i, value)| match value {
        &ValueType::I32 => RuntimeValue::I32(
          0, /* program_args[i]
								.parse::<i32>()
								.expect(&format!("Can't parse arg #{} as i32", program_args[i])),*/
        ),
        &ValueType::I64 => RuntimeValue::I64(
          0, /*  program_args[i]
									 .parse::<i64>()
									 .expect(&format!("Can't parse arg #{} as i64", program_args[i])),*/
        ),
        &ValueType::F32 => RuntimeValue::F32(
          0.0, /* program_args[i]
										.parse::<f32>()
										.expect(&format!("Can't parse arg #{} as f32", program_args[i])),*/
        ),
        &ValueType::F64 => RuntimeValue::F64(
          0.0, /*  program_args[i]
										 .parse::<f64>()
										 .expect(&format!("Can't parse arg #{} as f64", program_args[i])),*/
        ),
      })
      .collect::<Vec<RuntimeValue>>()
  };

  wasmi::Module::from_parity_wasm_module(module).expect("Module to be valid")
}
