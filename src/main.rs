extern crate parity_wasm;
extern crate wasmi;
extern crate rouille;
extern crate slab;
extern crate toml;
#[macro_use]
extern crate serde_derive;

use std::env::args;

use parity_wasm::elements::{External, FunctionType, Internal, Type, ValueType};
use wasmi::{ImportsBuilder, ModuleInstance, NopExternals, RuntimeValue};

mod host;
mod vm;
mod config;

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
