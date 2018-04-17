use std::ptr;
use std::str;

extern {
  fn log(ptr: *const u8, size: u64);
  fn response_set_status_line(status: u32, ptr: *const u8, size: u64);
  fn response_set_header(name_ptr: *const u8, name_size: u64, value_ptr: *const u8, value_size: u64);
  fn response_set_body(ptr: *const u8, size: u64);
  fn tcp_connect(ptr: *const u8, size: u64) -> i32;
  fn tcp_read(fd: i32, ptr: *mut u8, size: u64) -> i64;
  fn tcp_write(fd: i32, ptr: *const u8, size: u64) -> i64;
}


#[no_mangle]
pub extern "C" fn hello() {
  let s = b"Hello world!";
  unsafe { log(s.as_ptr(), s.len() as u64) };

  let status = 200;
  let reason = "Ok";
  unsafe {
    response_set_status_line(status, reason.as_ptr(), reason.len() as u64);
  };

  let body = "Hello world from wasm!\n";

  let header_name = "Content-length";
  let header_value = body.len().to_string();

  unsafe {
    response_set_header(header_name.as_ptr(), header_name.len() as u64, header_value.as_ptr(), header_value.len() as u64);
  };

  unsafe {
    response_set_body(body.as_ptr(), body.len() as u64);
  }
}

#[no_mangle]
pub extern "C" fn bonjour() {
  let s = b"Bonjour tout le monde!";
  unsafe { log(s.as_ptr(), s.len() as u64) };

  let status = 200;
  let reason = "Ok";
  unsafe {
    response_set_status_line(status, reason.as_ptr(), reason.len() as u64);
  };

  let body = "Bonjour tout le monde depuis le monde merveilleux de WASM!\n";

  let header_name = "Content-length";
  let header_value = body.len().to_string();

  unsafe {
    response_set_header(header_name.as_ptr(), header_name.len() as u64, header_value.as_ptr(), header_value.len() as u64);
  };

  unsafe {
    response_set_body(body.as_ptr(), body.len() as u64);
  }
}
