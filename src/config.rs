use std::fs::File;
use std::io::Read;
use toml;

#[derive(Deserialize,Debug)]
pub struct WasmApp {
  pub file_path: String,
  pub method: String,
  pub url_path: String,
  pub function: String,
}

#[derive(Deserialize,Debug)]
pub struct Config {
  pub listen_address: String,
  pub applications: Vec<WasmApp>,
}

pub fn load(file: &str) -> Option<Config> {
  if let Ok(mut file) = File::open(file) {
    let mut contents = String::new();
    if let Ok(_) = file.read_to_string(&mut contents) {
      return toml::from_str(&contents).ok()
    }
  }
  None
}
