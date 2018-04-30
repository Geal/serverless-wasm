use config::{ApplicationState, Config};

use mio::*;
use mio::net::{TcpListener, TcpStream};
use mio::unix::UnixReady;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::VecDeque;
use slab::Slab;

mod host;
mod session;

const SERVER: Token = Token(0);

pub fn server(config: Config) {
  let state = ApplicationState::new(&config);

  let addr = (&config.listen_address).parse().unwrap();
  let server = TcpListener::bind(&addr).unwrap();

  let mut poll = Poll::new().unwrap();

  poll
    .register(&server, SERVER, Ready::readable(), PollOpt::edge())
    .unwrap();

  let mut events = Events::with_capacity(1024);

  let state = Rc::new(RefCell::new(state));
  let mut connections = Slab::with_capacity(1024);
  let mut ready = VecDeque::new();

  loop {
    poll.poll(&mut events, None).unwrap();
    println!("got events: {:?}", events);

    for event in events.iter() {
      match event.token() {
        SERVER => {
          if let Ok((sock, addr)) = server.accept() {
            match connections.vacant_entry() {
              None => {
                println!("error: no more room for new connections");
              }
              Some(entry) => {
                let index = entry.index();
                poll.register(
                  &sock,
                  Token(index + 1),
                  Ready::readable() | Ready::writable() | Ready::from(UnixReady::hup() | UnixReady::error()),
                  PollOpt::edge(),
                );

                let client = Rc::new(RefCell::new(session::Session::new(
                  state.clone(),
                  sock,
                  index,
                )));
                entry.insert(client);
              }
            }
          }
        }
        Token(i) => {
          let client_token = i - 1;

          if let Some(ref mut client) = connections.get_mut(client_token) {
            if client
              .borrow_mut()
              .process_events(client_token, event.readiness())
            {
              ready.push_back(client_token);
            }
          } else {
            println!(
              "non existing token {:?} got events {:?}",
              client_token,
              event.readiness()
            );
          }
        }
        _ => unreachable!(),
      }
    }

    for client_token in ready.drain(..) {
      let mut cont = session::ExecutionResult::Continue;
      if let Some(ref mut client) = connections.get_mut(client_token) {
        cont = client.borrow_mut().execute();
      } else {
        println!("non existing token {:?} was marked as ready", client_token);
      }

      match cont {
        session::ExecutionResult::Close(tokens) => {
          for t in tokens.iter() {
            connections.remove(client_token);
          }
        },
        session::ExecutionResult::ConnectBackend(address) => {
          let client = connections.get(client_token).unwrap().clone();

          match connections.vacant_entry() {
            None => {
              println!("error: no more room for new connections");
            }
            Some(entry) => {
              let index = entry.index();
              let stream = TcpStream::connect(&address).unwrap();
              poll.register(
                &stream,
                Token(index + 1),
                Ready::readable() | Ready::writable() | Ready::from(UnixReady::hup() | UnixReady::error()),
                PollOpt::edge(),
              );
              client.borrow_mut().add_backend(stream, index);

              entry.insert(client);
            }
          }
        },
        _  => {}
      }
    }
  }
}
