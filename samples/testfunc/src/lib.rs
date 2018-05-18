use std::ptr;
use std::str;
extern crate serverless_api as api;

#[no_mangle]
pub extern "C" fn hello() {
  api::log("Hello world with api!");

  api::response::set_status(200, "Ok");

  let body = "Hello world from wasm!\n";
  api::response::set_header("Content-length", &body.len().to_string());
  api::response::set_body(body.as_bytes());
}

#[no_mangle]
pub extern "C" fn bonjour() {
  api::log("Bonjour tout le monde!");

  api::response::set_status(200, "Ok");

  let body = "Bonjour tout le monde depuis le monde merveilleux de WASM!\n";
  api::response::set_header("Content-length", &body.len().to_string());
  api::response::set_body(body.as_bytes());
}
