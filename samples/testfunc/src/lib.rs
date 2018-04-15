use std::ptr;

extern {
  fn log(ptr: *const u8, size: u64);
  fn response_set_status_line(status: u32, ptr: *const u8, size: u64);
  fn response_set_header(name_ptr: *const u8, name_size: u64, value_ptr: *const u8, value_size: u64);
  fn response_set_body(ptr: *const u8, size: u64);
}


#[no_mangle]
pub extern "C" fn test() {
  let s = b"Hello world!";
  unsafe { log(s.as_ptr(), s.len() as u64) };

  let status = 200;
  let reason = "Ok";
  unsafe {
    response_set_status_line(status, reason.as_ptr(), reason.len() as u64);
  };

  let body = "Hello world";

  let header_name = "Content-length";
  let header_value = body.len().to_string();

  unsafe {
    response_set_header(header_name.as_ptr(), header_name.len() as u64, header_value.as_ptr(), header_value.len() as u64);
  };

  unsafe {
    response_set_body(body.as_ptr(), body.len() as u64);
  }
}
