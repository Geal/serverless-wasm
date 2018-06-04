use config::Config;

use cretonne_wasm::{translate_module, DummyEnvironment};
use std::fs::File;
use std::io::Read;

mod env;

pub fn server(config: Config) {
  for app in config.applications.iter() {
    println!("loading {}:{} at '{} {}'", app.file_path, app.function, app.method, app.url_path);
    if let Ok(mut file) = File::open(&app.file_path) {
      let mut data = Vec::new();
      file.read_to_end(&mut data);

      //let mut env = DummyEnvironment::default();

      let mut env = env::Env::new();

      translate_module(&data, &mut env).unwrap();

      //let func_env = env.func_env();
      //println!("bytecode:\n{:?}", env.func_bytecode_sizes);
    }
  }
}
