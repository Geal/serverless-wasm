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
  fn db_get(key_ptr: *const u8, key_size: u64, value_ptr: *const u8, value_size: u64) -> i64;
}


#[no_mangle]
pub extern "C" fn handle() {
  let s = b"Hello world!";
  unsafe { log(s.as_ptr(), s.len() as u64) };

  let status = 200;
  let reason = "Ok";
  unsafe {
    response_set_status_line(status, reason.as_ptr(), reason.len() as u64);
  };

  let mut body = String::new();

  let addr_key = b"/env/backend";
  let mut addr: [u8; 100] = [0u8; 100];
  let read_sz = unsafe { db_get(addr_key.as_ptr(), addr_key.len() as u64, addr.as_mut_ptr(), addr.len() as u64) };
  if read_sz < 0 {
    let body = format!("could not get value for key {}", str::from_utf8(addr_key).unwrap());
    unsafe { log(body.as_ptr(), body.len() as u64) };

    let header_name = "Content-length";
    let header_value = body.len().to_string();

    unsafe {
      response_set_header(header_name.as_ptr(), header_name.len() as u64, header_value.as_ptr(), header_value.len() as u64);
    };

    unsafe {
      response_set_body(body.as_ptr(), body.len() as u64);
    }
    return;
  }
  //let addr = "127.0.0.1:8181";
  let backend = unsafe { tcp_connect(addr.as_ptr(), read_sz as u64) };

  if backend == -1 {
    body = format!("could not connect to backend address: {:?}\n", str::from_utf8(&addr[..read_sz as usize]));
    unsafe { log(body.as_ptr(), body.len() as u64) };
  } else {
    let backend_msg = "hello\n";
    let write_sz = unsafe { tcp_write(backend, backend_msg.as_ptr(), backend_msg.len() as u64) };

    if write_sz == -1 {
      body = String::from("could not write to backend server");
      unsafe { log(body.as_ptr(), body.len() as u64) };
    } else {
      let mut res: [u8; 100] = [0u8; 100];
      let read_sz = unsafe { tcp_read(backend, res.as_mut_ptr(), res.len() as u64) };

      if read_sz == -1 {
        body = String::from("could not read from backend server");
        unsafe { log(body.as_ptr(), body.len() as u64) };
      } else {
        let message = format!("read {} bytes from backend", read_sz);
        unsafe { log(message.as_ptr(), message.len() as u64) };

        body = format!("Hello world from wasm!\nanswer from backend:\n{}\n", str::from_utf8(&res[..]).unwrap());
      }
    }
  }

  let header_name = "Content-length";
  let header_value = body.len().to_string();

  unsafe {
    response_set_header(header_name.as_ptr(), header_name.len() as u64, header_value.as_ptr(), header_value.len() as u64);
  };

  unsafe {
    response_set_body(body.as_ptr(), body.len() as u64);
  }
}
