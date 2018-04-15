use std::ptr;

extern {
  fn log(ptr: *const u8, size: u64);
}


#[no_mangle]
pub extern "C" fn test() {
  let s = b"Hello world!";
  unsafe { log(s.as_ptr(), s.len() as u64) };
  /*unsafe {
    ptr::copy_nonoverlapping(s.as_ptr(), 0 as *mut u8, s.len());
  }

  unsafe { log(0 as *mut u8, s.len() as u64) };
  */
}
