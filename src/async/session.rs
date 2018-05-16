use mio::unix::UnixReady;
use mio::net::TcpStream;
use mio::{Poll, Ready};
use std::collections::HashMap;
use std::iter::repeat;
use std::rc::Rc;
use std::io::{ErrorKind, Read, Write};
use std::cell::RefCell;
use std::net::{SocketAddr, Shutdown};
use slab::Slab;

use interpreter::WasmInstance;
use super::host;
use config::ApplicationState;
use httparse;
use wasmi::{ExternVal, ImportsBuilder, ModuleInstance, TrapKind, RuntimeValue};

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionResult {
  WouldBlock,
  Close(Vec<usize>),
  Continue,
  ConnectBackend(SocketAddr),
  //Register(usize),
  //Remove(Vec<usize>),
}

#[derive(Debug)]
pub struct Stream {
  pub readiness: UnixReady,
  pub interest: UnixReady,
  pub stream: TcpStream,
  pub index: usize,
}

pub struct Buf {
  buf: Vec<u8>,
  offset: usize,
  len: usize,
}

#[derive(Debug,Clone,PartialEq)]
pub enum SessionState {
  WaitingForRequest,
  WaitingForBackendConnect(usize),
  Executing,
  Done,
}

pub struct Session {
  client: Stream,
  backends: HashMap<usize, Stream>,
  instance: Option<WasmInstance<host::State, host::AsyncHost>>,
  config: Rc<RefCell<ApplicationState>>,
  buffer: Buf,
  pub state: Option<SessionState>,
  method: Option<String>,
  path: Option<String>,
}

impl Session {
  pub fn new(config: Rc<RefCell<ApplicationState>>, stream: TcpStream, index: usize) -> Session {
    let client = Stream {
      readiness: UnixReady::from(Ready::empty()),
      interest: UnixReady::from(Ready::readable()) | UnixReady::hup() | UnixReady::error(),
      stream,
      index,
    };

    let capacity = 8192;
    let mut v = Vec::with_capacity(capacity);
    v.extend(repeat(0).take(capacity));
    let buffer = Buf {
      buf: v,
      offset: 0,
      len: 0,
    };

    Session {
      client,
      backends: HashMap::new(),
      instance: None,
      config,
      buffer,
    }
  }

  pub fn add_backend(&mut self, stream: TcpStream, index: usize) {
    let s = Stream {
      readiness: UnixReady::from(Ready::empty()),
      interest: UnixReady::from(Ready::writable()) | UnixReady::hup() | UnixReady::error(),
      stream,
      index,
    };

    self.backends.insert(index, s);

    self.state = Some(SessionState::WaitingForBackendConnect(index));
  }

  pub fn resume(&mut self)  -> ExecutionResult {
    let res = self.instance.as_mut().map(|instance| instance.resume()).unwrap();
    println!("resume result: {:?}", res);
    match res {
      Err(t) => match t.kind() {
        TrapKind::Host(ref err) => {
          match err.as_ref().downcast_ref() {
            Some(host::AsyncHostError::Connecting(address)) => {
              println!("returning connect to backend server: {}", address);
              return ExecutionResult::ConnectBackend(address.clone());
            },
            _ => { panic!("got host error: {:?}", err) }
          }
        },
        _ => {
          panic!("got trap: {:?}", t);
        }
      },
      Ok(_) => if self
        .instance
        .as_mut()
        .map(|instance| {
          println!(
            "set up response: {:?}",
            instance.state.borrow().prepared_response
          );
          instance
            .state
            .borrow()
            .prepared_response
            .status_code
            .is_some() && instance.state.borrow().prepared_response.body.is_some()
        })
        .unwrap_or(false)
      {
        self.client.interest.insert(Ready::writable());
        return ExecutionResult::Continue
      }
    }

    ExecutionResult::Continue
  }

  pub fn create_instance(&mut self) -> ExecutionResult {
    let method = self.method.as_ref().unwrap();
    let path = self.path.as_ref().unwrap();
    if let Some((func_name, module, ref opt_env)) = self.config.borrow().route(method, path) {
      let mut env = host::State::new();
      if let Some(h) = opt_env {
        env.db.extend(
          h.iter()
            .map(|(ref k, ref v)| (k.to_string(), v.to_string())),
        );
      }

      let main = ModuleInstance::new(&module, &ImportsBuilder::new().with_resolver("env", &env))
        .expect("Failed to instantiate module")
        .assert_no_start();

      if let Some(ExternVal::Func(func_ref)) = main.export_by_name(func_name) {
        let instance = WasmInstance::new(env, &func_ref, &[]);
        self.instance = Some(instance);
        ExecutionResult::Continue
      } else {
        println!("function not found");
        self
          .client
          .stream
          .write(b"404 Not Found\r\nContent-length: 19\r\n\r\nFunction not found\n");
        self.client.stream.shutdown(Shutdown::Both);
        self.client.interest = UnixReady::from(Ready::empty());
        ExecutionResult::Close(vec![self.client.index])
      }
    } else {
      println!("route not found");
      self
        .client
        .stream
        .write(b"404 Not Found\r\nContent-length: 16\r\n\r\nRoute not found\n");
      self.client.stream.shutdown(Shutdown::Both);
      self.client.interest = UnixReady::from(Ready::empty());
      ExecutionResult::Close(vec![self.client.index])
    }
  }

  pub fn process_events(&mut self, token: usize, events: Ready) -> bool {
    println!("client[{}]:  token {} got events {:?}", self.client.index, token, events);
    if token == self.client.index {
      self.client.readiness = self.client.readiness | UnixReady::from(events);

      self.client.readiness & self.client.interest != UnixReady::from(Ready::empty())
    } else {
      if let Some(ref mut stream) = self.backends.get_mut(&token) {
        println!("state: {:?}", self.state);
        if self.state == Some(SessionState::WaitingForBackendConnect(token)) {
          self.instance.as_mut().map(|instance| instance.add_function_result(RuntimeValue::I32(token as i32)));
          self.state = Some(SessionState::Executing);
        }

        stream.readiness.insert(UnixReady::from(events));
        stream.readiness & stream.interest != UnixReady::from(Ready::empty())
      } else {
        println!("non existing backend {} got events {:?}", token, events);
        false
      }
    }
  }

  pub fn execute(&mut self) -> ExecutionResult {
    loop {
      let front_readiness = self.client.readiness & self.client.interest;

      if front_readiness.is_readable() {
        let res = self.front_readable();
        if res != ExecutionResult::Continue {
          return res;
        }
      }

      let res = self.process();
      if res != ExecutionResult::Continue {
        return res;
      }

      if front_readiness.is_writable() {
        let res = self.front_writable();
        if res != ExecutionResult::Continue {
          return res;
        }
      }
    }
  }

  fn front_readable(&mut self) -> ExecutionResult {
    if self.state == Some(SessionState::WaitingForRequest) {
      loop {
        if self.buffer.offset + self.buffer.len == self.buffer.buf.len() {
          break;
        }

        match self
          .client
          .stream
          .read(&mut self.buffer.buf[self.buffer.offset + self.buffer.len..])
        {
          Ok(0) => {
            return ExecutionResult::Close(vec![self.client.index]);
          }
          Ok(sz) => {
            self.buffer.len += sz;
          }
          Err(e) => {
            if e.kind() == ErrorKind::WouldBlock {
              self.client.readiness.remove(Ready::readable());
              break;
            }
          }
        }
      }

      ExecutionResult::Continue
    } else {
      ExecutionResult::Close(vec![self.client.index])
    }
  }

  fn process(&mut self) -> ExecutionResult {
    println!("[{}] process", self.client.index);

    let state = self.state.take().unwrap();
    match state {
      SessionState::WaitingForRequest => {

        let (method, path) = {
          let mut headers = [httparse::Header {
            name: "",
            value: &[],
          }; 16];
          let mut req = httparse::Request::new(&mut headers);
          match req.parse(&self.buffer.buf[self.buffer.offset..self.buffer.len]) {
            Err(e) => {
              println!("http parsing error: {:?}", e);
              self.state = Some(SessionState::WaitingForRequest);
              return ExecutionResult::Close(vec![self.client.index]);
            }
            Ok(httparse::Status::Partial) => {
              self.state = Some(SessionState::WaitingForRequest);
              return ExecutionResult::Continue;
            }
            Ok(httparse::Status::Complete(sz)) => {
              self.buffer.offset += sz;
              println!("got request: {:?}", req);
              (
                req.method.unwrap().to_string(),
                req.path.unwrap().to_string(),
              )
            }
          }
        };

        self.client.interest.remove(Ready::readable());
        self.method = Some(method);
        self.path   = Some(path);
        self.state  = Some(SessionState::Executing);
        ExecutionResult::Continue
      },
      SessionState::Executing => {
        if self.instance.is_none() {
          let res = self.create_instance();
          if res != ExecutionResult::Continue {
            self.state = Some(SessionState::Executing);
            return res;
          }
        }

        println!("resuming");
        self.state = Some(SessionState::Executing);
        self.resume()
      },
      SessionState::WaitingForBackendConnect(_) => {
        panic!("should not have called execute() in WaitingForBackendConnect");
      },
      SessionState::Done => {
        panic!("done");
      }
    }
  }

  fn front_writable(&mut self) -> ExecutionResult {
    println!("[{}] front writable", self.client.index);
    let response = self
      .instance
      .as_mut()
      .map(|instance| instance.state.borrow().prepared_response.clone())
      .unwrap();

    self
      .client
      .stream
      .write_fmt(format_args!("{} Reason\r\n", response.status_code.unwrap()));
    for header in response.headers.iter() {
      self
        .client
        .stream
        .write_fmt(format_args!("{}: {}\r\n", header.0, header.1));
    }
    self.client.stream.write(b"\r\n");
    self.client.stream.write(&response.body.unwrap()[..]);

    ExecutionResult::Close(vec![self.client.index])
  }
}
