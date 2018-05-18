//! from https://github.com/paritytech/wasmi/blob/master/src/tests/host.rs

use slab::Slab;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::iter::repeat;
use std::net::TcpStream;
use std::str;
use std::cmp;
use std::rc::Rc;
use std::cell::RefCell;
use wasmi::memory_units::Pages;
use wasmi::*;
use interpreter::Host;

#[derive(Debug, Clone, PartialEq)]
struct HostErrorWithCode {
  error_code: u32,
}

impl ::std::fmt::Display for HostErrorWithCode {
  fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
    write!(f, "{}", self.error_code)
  }
}

impl HostError for HostErrorWithCode {}

#[derive(Clone)]
pub struct PreparedResponse {
  pub status_code: Option<u16>,
  pub headers: Vec<(String, String)>,
  pub body: Option<Vec<u8>>,
}

impl PreparedResponse {
  pub fn new() -> PreparedResponse {
    PreparedResponse {
      status_code: None,
      headers: Vec::new(),
      body: None,
    }
  }
}

pub struct State {
  memory: Option<MemoryRef>,
  instance: Option<ModuleRef>,
  pub prepared_response: PreparedResponse,
  connections: Slab<TcpStream>,
  pub db: HashMap<String, String>,
}

impl State {
  pub fn new() -> State {
    State {
      memory: Some(MemoryInstance::alloc(Pages(3), Some(Pages(10))).unwrap()),
      instance: None,
      prepared_response: PreparedResponse::new(),
      connections: Slab::with_capacity(100),
      db: HashMap::new(),
    }
  }
}

pub struct SyncHost {
  pub inner: Rc<RefCell<State>>,
}

impl Host for SyncHost {
  type State = State;

  fn build(s: Rc<RefCell<Self::State>>) -> Self {
    SyncHost { inner: s }
  }
}

/// log(ptr: *mut u8, size: u64)
///
/// Returns value at the given address in memory. This function
/// requires attached memory.
const LOG_INDEX: usize = 0;

const RESPONSE_SET_STATUS_LINE: usize = 1;
const RESPONSE_SET_HEADER: usize = 2;
const RESPONSE_SET_BODY: usize = 3;
const TCP_CONNECT: usize = 4;
const TCP_READ: usize = 5;
const TCP_WRITE: usize = 6;
const DB_GET: usize = 7;

impl Externals for SyncHost {
  fn invoke_index(&mut self, index: usize, args: RuntimeArgs) -> Result<Option<RuntimeValue>, Trap> {
    match index {
      LOG_INDEX => {
        let ptr: u32 = args.nth(0);
        let sz: u64 = args.nth(1);

        let v = self
          .inner
          .borrow()
          .memory
          .as_ref()
          .expect("Function 'inc_mem' expects attached memory")
          .get(ptr, sz as usize)
          .unwrap();

        println!("log({} bytes): {}", v.len(), str::from_utf8(&v).unwrap());
        Ok(None)
      }
      RESPONSE_SET_STATUS_LINE => {
        let status: u32 = args.nth(0);
        let ptr: u32 = args.nth(1);
        let sz: u64 = args.nth(2);

        let _reason = self
          .inner
          .borrow()
          .memory
          .as_ref()
          .expect("Function 'inc_mem' expects attached memory")
          .get(ptr, sz as usize)
          .unwrap();

        self.inner.borrow_mut().prepared_response.status_code = Some(status as u16);

        Ok(None)
      }
      RESPONSE_SET_HEADER => {
        let ptr1: u32 = args.nth(0);
        let sz1: u64 = args.nth(1);
        let ptr2: u32 = args.nth(2);
        let sz2: u64 = args.nth(3);
        let header_name = {
          self
            .inner
            .borrow()
            .memory
            .as_ref()
            .expect("Function 'inc_mem' expects attached memory")
            .get(ptr1, sz1 as usize)
            .unwrap()
        };
        let header_value = {
          self
            .inner
            .borrow()
            .memory
            .as_ref()
            .expect("Function 'inc_mem' expects attached memory")
            .get(ptr2, sz2 as usize)
            .unwrap()
        };

        self.inner.borrow_mut().prepared_response.headers.push((
          String::from_utf8(header_name).unwrap(),
          String::from_utf8(header_value).unwrap(),
        ));
        Ok(None)
      }
      RESPONSE_SET_BODY => {
        let ptr: u32 = args.nth(0);
        let sz: u64 = args.nth(1);

        let body = self
          .inner
          .borrow()
          .memory
          .as_ref()
          .expect("Function 'inc_mem' expects attached memory")
          .get(ptr, sz as usize)
          .unwrap();
        self.inner.borrow_mut().prepared_response.body = Some(body);
        Ok(None)
      }
      TCP_CONNECT => {
        let ptr: u32 = args.nth(0);
        let sz: u64 = args.nth(1);

        let v = self
          .inner
          .borrow()
          .memory
          .as_ref()
          .expect("Function 'inc_mem' expects attached memory")
          .get(ptr, sz as usize)
          .unwrap();
        let address = String::from_utf8(v).unwrap();
        if let Ok(socket) = TcpStream::connect(&address) {
          if let Ok(fd) = self.inner.borrow_mut().connections.insert(socket) {
            Ok(Some(RuntimeValue::I32(fd as i32)))
          } else {
            Ok(Some(RuntimeValue::I32(-2)))
          }
        } else {
          Ok(Some(RuntimeValue::I32(-1)))
        }
      }
      TCP_READ => {
        let fd: i32 = args.nth(0);
        let ptr: u32 = args.nth(1);
        let sz: u64 = args.nth(2);
        let mut v = Vec::with_capacity(sz as usize);
        v.extend(repeat(0).take(sz as usize));
        let mut state = self.inner.borrow_mut();
        if let Ok(sz) = state.connections[fd as usize].read(&mut v) {
          state.memory.as_ref().map(|m| m.set(ptr, &v[..sz]));

          Ok(Some(RuntimeValue::I64(sz as i64)))
        } else {
          Ok(Some(RuntimeValue::I64(-1)))
        }
      }
      TCP_WRITE => {
        let fd: i32 = args.nth(0);
        let ptr: u32 = args.nth(1);
        let sz: u64 = args.nth(2);

        let buf = self
          .inner
          .borrow()
          .memory
          .as_ref()
          .expect("Function 'inc_mem' expects attached memory")
          .get(ptr, sz as usize)
          .unwrap();

        if let Ok(sz) = self.inner.borrow_mut().connections[fd as usize].write(&buf) {
          Ok(Some(RuntimeValue::I64(sz as i64)))
        } else {
          Ok(Some(RuntimeValue::I64(-1)))
        }
      }
      DB_GET => {
        let key_ptr: u32 = args.nth(0);
        let key_sz: u64 = args.nth(1);
        let value_ptr: u32 = args.nth(2);
        let value_sz: u64 = args.nth(3);

        let v = self
          .inner
          .borrow()
          .memory
          .as_ref()
          .expect("Function 'inc_mem' expects attached memory")
          .get(key_ptr, key_sz as usize)
          .unwrap();
        let key = String::from_utf8(v).unwrap();
        println!("requested value for key {}", key);

        match self.inner.borrow().db.get(&key) {
          None => Ok(Some(RuntimeValue::I64(-1))),
          Some(value) => {
            let to_write = cmp::min(value.len(), value_sz as usize);
            self
              .inner
              .borrow()
              .memory
              .as_ref()
              .map(|m| m.set(value_ptr, (&value[..to_write]).as_bytes()));
            Ok(Some(RuntimeValue::I64(value.len() as i64)))
          }
        }
      }
      _ => panic!("env doesn't provide function at index {}", index),
    }
  }
}

impl State {
  fn check_signature(&self, index: usize, signature: &Signature) -> bool {
    let (params, ret_ty): (&[ValueType], Option<ValueType>) = match index {
      LOG_INDEX => (&[ValueType::I32, ValueType::I64], None),
      RESPONSE_SET_STATUS_LINE => (&[ValueType::I32, ValueType::I32, ValueType::I64], None),
      RESPONSE_SET_HEADER => (
        &[
          ValueType::I32,
          ValueType::I64,
          ValueType::I32,
          ValueType::I64,
        ],
        None,
      ),
      RESPONSE_SET_BODY => (&[ValueType::I32, ValueType::I64], None),
      TCP_CONNECT => (&[ValueType::I32, ValueType::I64], Some(ValueType::I32)),
      TCP_READ => (
        &[ValueType::I32, ValueType::I32, ValueType::I64],
        Some(ValueType::I64),
      ),
      TCP_WRITE => (
        &[ValueType::I32, ValueType::I32, ValueType::I64],
        Some(ValueType::I64),
      ),
      DB_GET => (
        &[
          ValueType::I32,
          ValueType::I64,
          ValueType::I32,
          ValueType::I64,
        ],
        Some(ValueType::I64),
      ),
      _ => return false,
    };

    signature.params() == params && signature.return_type() == ret_ty
  }
}

impl ModuleImportResolver for State {
  fn resolve_func(&self, field_name: &str, signature: &Signature) -> Result<FuncRef, Error> {
    let index = match field_name {
      "log" => LOG_INDEX,
      "response_set_status_line" => RESPONSE_SET_STATUS_LINE,
      "response_set_header" => RESPONSE_SET_HEADER,
      "response_set_body" => RESPONSE_SET_BODY,
      "tcp_connect" => TCP_CONNECT,
      "tcp_read" => TCP_READ,
      "tcp_write" => TCP_WRITE,
      "db_get" => DB_GET,
      _ => {
        return Err(Error::Instantiation(format!(
          "Export {} not found",
          field_name
        )))
      }
    };

    if !self.check_signature(index, signature) {
      return Err(Error::Instantiation(format!(
        "Export `{}` doesnt match expected type {:?}",
        field_name, signature
      )));
    }

    Ok(FuncInstance::alloc_host(signature.clone(), index))
  }

  fn resolve_memory(&self, _field_name: &str, _memory_type: &MemoryDescriptor) -> Result<MemoryRef, Error> {
    let Pages(initial1) = self.memory.as_ref().map(|m| m.initial()).unwrap();
    let initial2 = _memory_type.initial() as usize;
    //println!("requested {} pages", initial2);
    if initial2 > initial1 {
      self.memory.as_ref().map(|_m| {
        //println!("grow res: {:?}", m.grow(Pages(initial2 - initial1)).unwrap());
      });
    }
    let Pages(_initial) = self.memory.as_ref().map(|m| m.current_size()).unwrap();
    //println!("current number of pages: {}", initial);
    //println!("resolving memory at name: {}", field_name);
    let res = self.memory.as_ref().unwrap().clone();

    Ok(res)
  }
}
