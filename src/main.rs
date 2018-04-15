extern crate parity_wasm;
extern crate wasmi;
extern crate rouille;

use std::env::args;

use parity_wasm::elements::{External, FunctionType, Internal, Type, ValueType};
use wasmi::{ImportsBuilder, ModuleInstance, NopExternals, RuntimeValue};

mod host;
mod vm;

fn main() {
    let args: Vec<_> = args().collect();
    /*if args.len() < 3 {
        println!("Usage: {} <wasm file> <exported func> [<arg>...]", args[0]);
        return;
    }
    */

    vm::server(&args[1]);
}
