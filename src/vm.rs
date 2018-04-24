use parity_wasm;
use parity_wasm::elements::{External, FunctionType, Internal, Type, ValueType};
use wasmi::{self,ImportsBuilder, Module, ModuleInstance,
  RuntimeValue, Error, ExternVal};
use rouille;

use super::host;
use config::{Config, ApplicationState};
use interpreter::WasmInstance;

pub fn server(config: Config) {
    let state = ApplicationState::new(&config);

    rouille::start_server(&config.listen_address, move |request| {
        if let Some((func_name, module, ref opt_env)) = state.route(request.method(), &request.url()) {
          let mut env = host::TestHost::new();
          if let Some(h) = opt_env {
            env.db.extend(h.iter().map(|(ref k, ref v)| (k.to_string(), v.to_string())));
          }
          let main = ModuleInstance::new(&module, &ImportsBuilder::new().with_resolver("env", &env))
            .expect("Failed to instantiate module")
            .assert_no_start();

          if let Some(ExternVal::Func(func_ref)) = main.export_by_name(func_name) {
            let mut instance = WasmInstance::new(&mut env, &func_ref, &[]);
            let res = instance.resume().map_err(|t| Error::Trap(t));
            println!(
                "invocation result: {:?}",
                res
            );
          } else {
            panic!("handle error here");
          };

          if let host::PreparedResponse {
            status_code: Some(status), headers, body: Some(body)
          } = env.prepared_response {
            rouille::Response {
              status_code: status,
              headers: Vec::new(),
              data: rouille::ResponseBody::from_data(body),
              upgrade: None,
            }
          } else {
            rouille::Response::text("wasm failed").with_status_code(500)
          }
        } else {
          rouille::Response::empty_404()
        }
    });
}

/*
pub fn start(file: &str) {
    let module = load_module(file, "handle");
    let mut env = host::TestHost::new();
    let main = ModuleInstance::new(&module, &ImportsBuilder::new().with_resolver("env", &env))
      .expect("Failed to instantiate module")
      .assert_no_start();

    println!(
        "Result: {:?}",
        main.invoke_export("handle", &[], &mut env)
    );
}
*/

pub fn load_module(file:&str, func_name: &str) -> Module {
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
                .iter().map(|entry| {
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
        let func_type_ref: usize =
            function_section.entries()[function_index_in_section].type_ref() as usize;

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


