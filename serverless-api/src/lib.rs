use std::str;

mod sys {
  extern {
    pub fn log(ptr: *const u8, size: u64);
    pub fn response_set_status_line(status: u32, ptr: *const u8, size: u64);
    pub fn response_set_header(name_ptr: *const u8, name_size: u64, value_ptr: *const u8, value_size: u64);
    pub fn response_set_body(ptr: *const u8, size: u64);
    pub fn tcp_connect(ptr: *const u8, size: u64) -> i32;
    pub fn tcp_read(fd: i32, ptr: *mut u8, size: u64) -> i64;
    pub fn tcp_write(fd: i32, ptr: *const u8, size: u64) -> i64;
    pub fn db_get(key_ptr: *const u8, key_size: u64, value_ptr: *const u8, value_size: u64) -> i64;
  }
}

pub fn log(s: &str) {
  unsafe { sys::log(s.as_ptr(), s.len() as u64) };
}

pub mod db {
  use super::sys;
  use std::iter::repeat;

  pub fn get(key: &str) -> Option<String> {
    let mut empty = vec![];
    let read_sz = unsafe {
      sys::db_get(key.as_ptr(), key.len() as u64, (&mut empty).as_mut_ptr(), empty.len() as u64)
    };

    if read_sz < 0 {
      return None;
    } else if read_sz == 0 {
      return Some(String::new());
    }

    let mut v = Vec::with_capacity(read_sz as usize);
    v.extend(repeat(0).take(read_sz as usize));

    let sz = unsafe {
      sys::db_get(key.as_ptr(), key.len() as u64, v.as_mut_ptr(), v.len() as u64)
    };

    if sz < 0 {
      return None;
    } else if sz == 0 {
      return Some(String::new());
    }

    if sz as usize != v.len() {
      None
    } else {
      String::from_utf8(v).ok()
    }
  }
}

pub mod response {
  use super::sys;

  pub fn set_status(status: u16, reason: &str) {
    unsafe {
      sys::response_set_status_line(status.into(), reason.as_ptr(), reason.len() as u64);
    }
  }

  pub fn set_header(name: &str, value: &str) {
    unsafe {
      sys::response_set_header(name.as_ptr(), name.len() as u64, value.as_ptr(), value.len() as u64);
    }
  }

  pub fn set_body(body: &[u8]) {
    unsafe {
      sys::response_set_body(body.as_ptr(), body.len() as u64);
    }
  }
}

pub struct TcpStream {
  fd: i32
}

impl TcpStream {
  pub fn connect(address: &str) -> Option<TcpStream> {
    let fd = unsafe { sys::tcp_connect(address.as_ptr(), address.len() as u64) };
    if fd < 0 {
      None
    } else {
      Some(TcpStream { fd })
    }
  }

  pub fn write(&mut self, data: &[u8]) -> Option<usize> {
    let res = unsafe { sys::tcp_write(self.fd, data.as_ptr(), data.len() as u64) };
    if res < 0 {
      None
    } else {
      Some(res as usize)
    }
  }

  pub fn read(&mut self, data: &mut [u8]) -> Option<usize> {
    let res = unsafe { sys::tcp_read(self.fd, data.as_mut_ptr(), data.len() as u64) };
    if res < 0 {
      None
    } else {
      Some(res as usize)
    }
  }
}

