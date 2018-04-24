extern crate parity_wasm;
extern crate wasmi;
extern crate rouille;
extern crate slab;
extern crate toml;
#[macro_use]
extern crate serde_derive;

use std::env::args;


mod host;
mod vm;
mod config;
mod interpreter;

fn main() {
    let args: Vec<_> = args().collect();
    if args.len() != 2 {
        println!("Usage: {} <config_file>", args[0]);
        return;
    }

    if let Some(config) = config::load(&args[1]) {
      vm::server(config);
    } else {
      println!("invalid configuration");
    }
}
