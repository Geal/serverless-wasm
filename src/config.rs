use interpreter::load_module;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use toml;
use wasmi::Module;

#[derive(Deserialize, Debug)]
pub struct WasmApp {
  pub file_path: String,
  pub method: String,
  pub url_path: String,
  pub function: String,
  pub env: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug)]
pub struct Config {
  pub listen_address: String,
  pub applications: Vec<WasmApp>,
}

pub fn load(file: &str) -> Option<Config> {
  if let Ok(mut file) = File::open(file) {
    let mut contents = String::new();
    if let Ok(_) = file.read_to_string(&mut contents) {
      return toml::from_str(&contents)
        .map_err(|e| {
          println!("configuration deserialization error: {:?}", e);
          e
        })
        .ok();
    }
  }
  None
}

pub struct ApplicationState {
  /// (method, url path) -> (function name, module path, env)
  pub routes: HashMap<(String, String), (String, String, Option<HashMap<String, String>>)>,
  /// module path -> Module
  pub modules: HashMap<String, Module>,
}

impl ApplicationState {
  pub fn new(config: &Config) -> ApplicationState {
    let mut routes = HashMap::new();
    let mut modules = HashMap::new();

    for app in config.applications.iter() {
      //FIXME: it might be good to not panic when we don't find the function in the module
      let module = load_module(&app.file_path, &app.function);

      if !modules.contains_key(&app.file_path) {
        modules.insert(app.file_path.clone(), module);
      }

      routes.insert(
        (app.method.clone(), app.url_path.clone()),
        (app.function.clone(), app.file_path.clone(), app.env.clone()),
      );
    }

    ApplicationState {
      routes: routes,
      modules: modules,
    }
  }

  pub fn route(&self, method: &str, url: &str) -> Option<(&str, &Module, &Option<HashMap<String, String>>)> {
    if let Some((func_name, module_path, ref opt_env)) = self.routes.get(&(method.to_string(), url.to_string())) {
      if let Some(module) = self.modules.get(module_path) {
        return Some((func_name, module, opt_env));
      }
    }

    None
  }
}
