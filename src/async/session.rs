use mio::unix::UnixReady;
use mio::net::TcpStream;
use mio::{Poll, Ready};
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

use interpreter::WasmInstance;
use super::host;
use config::ApplicationState;

pub struct Stream {
  pub readiness: UnixReady,
  pub interest: UnixReady,
  pub stream: TcpStream,
  pub index: usize,
}

pub struct Session<'a> {
  client: Stream,
  backends: HashMap<usize, Stream>,
  instance: Option<WasmInstance<'a, host::AsyncHost>>,
  config: Rc<RefCell<ApplicationState>>,
}

impl<'a> Session<'a> {
  pub fn new(config: Rc<RefCell<ApplicationState>>, stream: TcpStream, index: usize,
    poll: &mut Poll) -> Session<'a> {

    let client = Stream {
      readiness: UnixReady::from(Ready::empty()),
      interest: UnixReady::from(Ready::readable()) | UnixReady::hup() | UnixReady::error(),
      stream,
      index,
    };

    Session {
      client,
      backends: HashMap::new(),
      instance: None,
      config,
    }
  }

  pub fn resume(&mut self) {
    self.instance.as_mut().map(|instance| instance.resume());
  }

  pub fn process_events(&mut self, token: usize, events: Ready) -> bool {
    if token == self.client.index {
      self.client.readiness = self.client.readiness | UnixReady::from(events);

      self.client.readiness & self.client.interest != UnixReady::from(Ready::empty())
    } else {
      if let Some(ref mut stream) = self.backends.get_mut(&token) {
        stream.readiness.insert(UnixReady::from(events));
        stream.readiness & stream.interest != UnixReady::from(Ready::empty())
      } else {
        println!("non existing backend {} got events {:?}", token, events);
        false
      }
    }
  }

  pub fn execute(&mut self, poll: &mut Poll) -> bool {
    true
  }
}
