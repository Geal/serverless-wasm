//! from https://github.com/paritytech/wasmi/blob/master/src/tests/host.rs

use wasmi::*;
use wasmi::memory_units::{Bytes, Pages};
use std::str;
use std::io::{Read,Write};
use std::iter::repeat;
use rouille::Response;
use slab::Slab;
use std::net::TcpStream;

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

pub struct TestHost {
    memory: Option<MemoryRef>,
    instance: Option<ModuleRef>,
    pub prepared_response: PreparedResponse,
    connections: Slab<TcpStream>,
}

impl TestHost {
  pub fn new() -> TestHost {
        TestHost {
            memory: Some(MemoryInstance::alloc(Pages(3), Some(Pages(10))).unwrap()),
            instance: None,
            prepared_response: PreparedResponse::new(),
            connections: Slab::new(),
        }
    }
}

/// sub(a: i32, b: i32) -> i32
///
/// This function just substracts one integer from another,
/// returning the subtraction result.
const SUB_FUNC_INDEX: usize = 0;

/// err(error_code: i32) -> !
///
/// This function traps upon a call.
/// The trap have a special type - HostErrorWithCode.
const ERR_FUNC_INDEX: usize = 1;

/// inc_mem(ptr: *mut u8)
///
/// Increments value at the given address in memory. This function
/// requires attached memory.
const INC_MEM_FUNC_INDEX: usize = 2;

/// get_mem(ptr: *mut u8) -> u8
///
/// Returns value at the given address in memory. This function
/// requires attached memory.
const GET_MEM_FUNC_INDEX: usize = 3;

/// recurse<T>(val: T) -> T
///
/// If called, resolves exported function named 'recursive' from the attached
/// module instance and then calls into it with the provided argument.
/// Note that this function is polymorphic over type T.
/// This function requires attached module instance.
const RECURSE_FUNC_INDEX: usize = 4;

/// log(ptr: *mut u8, size: u64)
///
/// Returns value at the given address in memory. This function
/// requires attached memory.
const LOG_INDEX: usize = 5;

const RESPONSE_SET_STATUS_LINE: usize = 6;
const RESPONSE_SET_HEADER: usize = 7;
const RESPONSE_SET_BODY: usize = 8;
const TCP_CONNECT: usize = 9;
const TCP_READ: usize = 10;
const TCP_WRITE: usize = 11;

impl Externals for TestHost {
    fn invoke_index(
        &mut self,
        index: usize,
        args: RuntimeArgs,
    ) -> Result<Option<RuntimeValue>, Trap> {
        match index {
            SUB_FUNC_INDEX => {
                let a: i32 = args.nth(0);
                let b: i32 = args.nth(1);

                let result: RuntimeValue = (a - b).into();

                Ok(Some(result))
            }
            ERR_FUNC_INDEX => {
                let error_code: u32 = args.nth(0);
                let error = HostErrorWithCode { error_code };
                Err(TrapKind::Host(Box::new(error)).into())
            }
            INC_MEM_FUNC_INDEX => {
                let ptr: u32 = args.nth(0);

                let memory = self.memory
                    .as_ref()
                    .expect("Function 'inc_mem' expects attached memory");
                let mut buf = [0u8; 1];
                memory.get_into(ptr, &mut buf).unwrap();
                buf[0] += 1;
                memory.set(ptr, &buf).unwrap();

                Ok(None)
            }
            GET_MEM_FUNC_INDEX => {
                let ptr: u32 = args.nth(0);

                let memory = self.memory
                    .as_ref()
                    .expect("Function 'get_mem' expects attached memory");
                let mut buf = [0u8; 1];
                memory.get_into(ptr, &mut buf).unwrap();

                Ok(Some(RuntimeValue::I32(buf[0] as i32)))
            }
            RECURSE_FUNC_INDEX => {
                let val = args.nth_value_checked(0)
                    .expect("Exactly one argument expected");

                let instance = self.instance
                    .as_ref()
                    .expect("Function 'recurse' expects attached module instance")
                    .clone();
                let result = instance
                    .invoke_export("recursive", &[val.into()], self)
                    .expect("Failed to call 'recursive'")
                    .expect("expected to be Some");

                if val.value_type() != result.value_type() {
                    return Err(
                        TrapKind::Host(Box::new(HostErrorWithCode { error_code: 123 })).into(),
                    );
                }
                Ok(Some(result))
            },
            LOG_INDEX => {
              let ptr: u32 = args.nth(0);
              let sz:  u64 = args.nth(1);
              println!("got args ptr={}, sz={}", ptr, sz);

              let byte_size: Bytes = self.memory.as_ref().map(|m| m.current_size().into()).unwrap();
              println!("full memory size: {:?}", byte_size);
              let memory = self.memory
                .as_ref()
                .expect("Function 'inc_mem' expects attached memory");
              let v = memory.get(ptr, sz as usize).unwrap();

              println!("log({} bytes):\n{:?}\n{}", v.len(), v, str::from_utf8(&v).unwrap());
              Ok(None)
            },
            RESPONSE_SET_STATUS_LINE => {
              let status: u32 = args.nth(0);
              let ptr: u32 = args.nth(1);
              let sz:  u64 = args.nth(2);

              let byte_size: Bytes = self.memory.as_ref().map(|m| m.current_size().into()).unwrap();
              let memory = self.memory
                .as_ref()
                .expect("Function 'inc_mem' expects attached memory");
              let reason = memory.get(ptr, sz as usize).unwrap();

              self.prepared_response.status_code = Some(status as u16);

              Ok(None)
            },
            RESPONSE_SET_HEADER => {
              let ptr1: u32 = args.nth(0);
              let sz1:  u64 = args.nth(1);
              let ptr2: u32 = args.nth(2);
              let sz2:  u64 = args.nth(3);
              let header_name = {
                let byte_size: Bytes = self.memory.as_ref().map(|m| m.current_size().into()).unwrap();
                let memory = self.memory
                  .as_ref()
                  .expect("Function 'inc_mem' expects attached memory");
                memory.get(ptr1, sz1 as usize).unwrap()
              };
              let header_value = {
                let byte_size: Bytes = self.memory.as_ref().map(|m| m.current_size().into()).unwrap();
                let memory = self.memory
                  .as_ref()
                  .expect("Function 'inc_mem' expects attached memory");
                memory.get(ptr2, sz2 as usize).unwrap()
              };

              self.prepared_response.headers.push((
                String::from_utf8(header_name).unwrap(),
                String::from_utf8(header_value).unwrap()
              ));
              Ok(None)
            },
            RESPONSE_SET_BODY => {
              let ptr: u32 = args.nth(0);
              let sz:  u64 = args.nth(1);

              let byte_size: Bytes = self.memory.as_ref().map(|m| m.current_size().into()).unwrap();
              let memory = self.memory
                .as_ref()
                .expect("Function 'inc_mem' expects attached memory");
              let body = memory.get(ptr, sz as usize).unwrap();
              self.prepared_response.body = Some(body);
              Ok(None)
            },
            TCP_CONNECT => {
              let ptr: u32 = args.nth(0);
              let sz:  u64 = args.nth(1);
              println!("got args ptr={}, sz={}", ptr, sz);

              let byte_size: Bytes = self.memory.as_ref().map(|m| m.current_size().into()).unwrap();
              println!("full memory size: {:?}", byte_size);
              let memory = self.memory
                .as_ref()
                .expect("Function 'inc_mem' expects attached memory");
              let v = memory.get(ptr, sz as usize).unwrap();
              let address = String::from_utf8(v).unwrap();
              let fd = self.connections.insert(TcpStream::connect(&address).unwrap());

              Ok(Some(RuntimeValue::I32(fd as i32)))
            },
            TCP_READ => {
              let fd: i32 = args.nth(0);
              let ptr: u32 = args.nth(1);
              let sz:  u64 = args.nth(2);
              let mut v = Vec::with_capacity(sz as usize);
              v.extend(repeat(0).take(sz as usize));
              let sz = self.connections[fd as usize].read(&mut v).unwrap();
              self.memory.as_ref().map(|m| m.set(ptr, &v[..sz]));


              Ok(None)
            },
            TCP_WRITE => {
              let fd: i32 = args.nth(0);
              let ptr: u32 = args.nth(1);
              let sz:  u64 = args.nth(2);

              let byte_size: Bytes = self.memory.as_ref().map(|m| m.current_size().into()).unwrap();
              let memory = self.memory
                .as_ref()
                .expect("Function 'inc_mem' expects attached memory");
              let buf = memory.get(ptr, sz as usize).unwrap();

              self.connections[fd as usize].write(&buf);

              Ok(None)
            },
            _ => panic!("env doesn't provide function at index {}", index),
        }
    }
}

impl TestHost {
    fn check_signature(&self, index: usize, signature: &Signature) -> bool {
        if index == RECURSE_FUNC_INDEX {
            // This function requires special handling because it is polymorphic.
            if signature.params().len() != 1 {
                return false;
            }
            let param_type = signature.params()[0];
            return signature.return_type() == Some(param_type);
        }

        let (params, ret_ty): (&[ValueType], Option<ValueType>) = match index {
            SUB_FUNC_INDEX => (&[ValueType::I32, ValueType::I32], Some(ValueType::I32)),
            ERR_FUNC_INDEX => (&[ValueType::I32], None),
            INC_MEM_FUNC_INDEX => (&[ValueType::I32], None),
            GET_MEM_FUNC_INDEX => (&[ValueType::I32], Some(ValueType::I32)),
            LOG_INDEX => (&[ValueType::I32, ValueType::I64], None),
            RESPONSE_SET_STATUS_LINE => (&[ValueType::I32, ValueType::I32, ValueType::I64], None),
            RESPONSE_SET_HEADER => (&[ValueType::I32, ValueType::I64, ValueType::I32, ValueType::I64], None),
            RESPONSE_SET_BODY => (&[ValueType::I32, ValueType::I64], None),
            TCP_CONNECT => (&[ValueType::I32, ValueType::I64], Some(ValueType::I32)),
            TCP_READ => (&[ValueType::I32, ValueType::I32, ValueType::I64], None),
            TCP_WRITE => (&[ValueType::I32, ValueType::I32, ValueType::I64], None),
            _ => return false,
        };

        signature.params() == params && signature.return_type() == ret_ty
    }
}

impl ModuleImportResolver for TestHost {
    fn resolve_func(&self, field_name: &str, signature: &Signature) -> Result<FuncRef, Error> {
        let index = match field_name {
            "sub" => SUB_FUNC_INDEX,
            "err" => ERR_FUNC_INDEX,
            "inc_mem" => INC_MEM_FUNC_INDEX,
            "get_mem" => GET_MEM_FUNC_INDEX,
            "recurse" => RECURSE_FUNC_INDEX,
            "log" => LOG_INDEX,
            "response_set_status_line" => RESPONSE_SET_STATUS_LINE,
            "response_set_header" => RESPONSE_SET_HEADER,
            "response_set_body" => RESPONSE_SET_BODY,
            "tcp_connect" => TCP_CONNECT,
            "tcp_read" => TCP_READ,
            "tcp_write" => TCP_WRITE,
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

    fn resolve_memory(
        &self,
        field_name: &str,
        _memory_type: &MemoryDescriptor,
    ) -> Result<MemoryRef, Error> {
      let Pages(initial1) = self.memory.as_ref().map(|m| m.initial()).unwrap();
      let initial2 = _memory_type.initial() as usize;
      println!("requested {} pages", initial2);
      if initial2 > initial1 {
        self.memory.as_ref().map(|m| {
          println!("grow res: {:?}", m.grow(Pages(initial2 - initial1)).unwrap());
        });
      }
      let Pages(initial) = self.memory.as_ref().map(|m| m.current_size()).unwrap();
      println!("current number of pages: {}", initial);
      println!("resolving memory at name: {}", field_name);
      let res = self.memory.as_ref().unwrap().clone();

      Ok(res)
    }
}
