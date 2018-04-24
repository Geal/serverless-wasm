use wasmi::{Externals, Error, Interpreter,FuncRef,FunctionContext,RuntimeValue,Trap,FuncInstance,RunResult,BlockFrameType};
use std::collections::VecDeque;

pub const DEFAULT_VALUE_STACK_LIMIT: usize = 16384;
pub const DEFAULT_FRAME_STACK_LIMIT: usize = 16384;

pub struct WasmInstance<'a, E: Externals + 'a> {
  pub interpreter: Interpreter<'a, E>,
  pub stack: VecDeque<FunctionContext>,
}

impl<'a, E: Externals> WasmInstance<'a, E> {
  pub fn new(env: &'a mut E, func_ref: &FuncRef, args: &[RuntimeValue]) -> WasmInstance<'a, E> {
    let mut interpreter = Interpreter::new(env);
    let mut stack = create_stack(&func_ref, &[]);

    WasmInstance { interpreter, stack }
  }

  pub fn resume(&mut self) -> Result<Option<RuntimeValue>, Trap> {
    my_run_interpreter_loop(&mut self.interpreter, &mut self.stack)
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

  pub fn my_run_interpreter_loop<'a, E>(interpreter: &mut Interpreter<'a,E>, function_stack: &mut VecDeque<FunctionContext>) -> Result<Option<RuntimeValue>, Trap>
    where E: Externals {
		loop {
			let mut function_context = function_stack.pop_back().expect("on loop entry - not empty; on loop continue - checking for emptiness; qed");
			let function_ref = function_context.function.clone();
			let function_body = function_ref
				.body()
				.expect(
					"Host functions checked in function_return below; Internal functions always have a body; qed"
				);
			if !function_context.is_initialized() {
				let return_type = function_context.return_type;
				function_context.initialize(&function_body.locals);
				function_context.push_frame(&function_body.labels, BlockFrameType::Function, return_type).map_err(Trap::new)?;
			}

			let function_return = interpreter.do_run_function(&mut function_context, function_body.opcodes.elements(), &function_body.labels).map_err(Trap::new)?;

			match function_return {
				RunResult::Return(return_value) => {
					match function_stack.back_mut() {
						Some(caller_context) => if let Some(return_value) = return_value {
							caller_context.value_stack_mut().push(return_value).map_err(Trap::new)?;
						},
						None => return Ok(return_value),
					}
				},
				RunResult::NestedCall(nested_func) => {
          println!("calling nested func");
          match FuncInstance::invoke_context(&nested_func, &mut function_context, interpreter.externals)? {
            None => {
							function_stack.push_back(function_context);
            },
            Some(nested_context) => {
							function_stack.push_back(function_context);
							function_stack.push_back(nested_context);
            }
          }
				},
			}
		}
	}
