use std::str;
extern crate serverless_api as api;

#[no_mangle]
pub extern "C" fn handle() {
  api::log("Hello world with api!");
  let body;

  let key = "/env/backend";
  match api::db::get(key) {
    None => {
      body = format!("could not get value for key {}", key);
    },
    Some(address) => {
      api::log(&format!("connecting to backend at {}", address));

      match api::TcpStream::connect(&address) {
        None => {
          body = "could not connect to backend".to_string();
        },
        Some(mut socket) => {
          match socket.write(b"hello\n") {
            None => {
              body = "could not write to backend server".to_string();
            },
            Some(_) => {
              let mut res: [u8; 100] = [0u8; 100];
              match socket.read(&mut res) {
                None => {
                  body = "could not read from backend server".to_string();
                },
                Some(sz) => {
                  api::log(&format!("read data from backend: \"{:?}\"", str::from_utf8(&res[..sz]).unwrap()));

                  body = format!("Hello world from wasm!\nanswer from backend:\n{}\n", str::from_utf8(&res[..sz]).unwrap());
                  api::response::set_status(200, "Ok");
                  api::response::set_header("Content-length", &body.len().to_string());
                  api::response::set_body(body.as_bytes());
                }
              }
            }
          }
        }
      }
    }
  }

  api::log(&body);
  api::response::set_status(500, "Server error");
  api::response::set_header("Content-length", &body.len().to_string());
  api::response::set_body(body.as_bytes());
}
