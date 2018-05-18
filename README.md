# Serverless Web Assembly framework

![Serverless WASM](https://raw.githubusercontent.com/geal/serverless-wasm/master/assets/serverless-wasm.jpg)

## Why?

For fun.

## But why?

![but why?](https://raw.githubusercontent.com/geal/serverless-wasm/master/assets/butwhy.gif)

This is a small demo of Web Assembly's potential outside of browsers.
It has been designed with client side execution in mind, but there's
nothing preventing it from running in other platforms.
There are people working on running WASM binaries from a shell, and
putting WASM code inside kernels.

Here are some benefits of Web Assembly's design:

- maps well to CPU assembly (with Just In Time compilation in mind)
- the virtual machine does not require garbage collection, only small memory areas
that will be handled by the guest code
- meant to be sandboxed

And here's the best part: it is meant to be a target language, a lot of other
languages will compile to WASM. You can already write C or C++ and compile
to WASM with emscripten. Rust's compiler natively supports it. There are demos
in Go, Haskell and Ruby.

The network effects are huge: the major browsers implement it and can run any
WASM app, and every language wants to run on the client side.

Now, what happens when you leverage these advantages to build a server platform?
You get a system that can run a lot of small, sandboxed, resource limited
applications, written in a lot of different languages.

You do not care about how to start it, you don't need to map it to filesystems
and common runtimes like containers do. You just have some bytecode that imports
a few functions, and runs well isolated.

This is a bit like the serverless promise, the ability to run arbitrarily small
functions, but without even caring about the startup time or the state size,
this will be the smallest you can think of.

## What works

Currently, the server is able to load a pre built web assembly binary, exports
some function that it can use for logging, to build a response and connect to
other servers, and handle requests using that wasm file (as long as it exports
a "handle" function).

## How to run it

### Requirements for WASM applications

the WASM application must export a `handle` function that takes no arguments and
returns no arguments.

The virtual machine currently exposes the following functions, that you can use
to build your response:

```rust
extern {
  fn log(ptr: *const u8, size: u64);

  fn response_set_status_line(status: u32, ptr: *const u8, size: u64);
  fn response_set_header(name_ptr: *const u8, name_size: u64, value_ptr: *const u8, value_size: u64);
  fn response_set_body(ptr: *const u8, size: u64);

  fn tcp_connect(ptr: *const u8, size: u64) -> i32;
  fn tcp_read(fd: i32, ptr: *mut u8, size: u64) -> i64;
  fn tcp_write(fd: i32, ptr: *const u8, size: u64) -> i64;
}
```

### Configuration file

You define which WASM binary will handle which requests through a TOML configuration
file:

```toml
listen_address = "127.0.0.1:8080"

[[applications]]
file_path = "./samples/testfunc.wasm"
method = "GET"
url_path = "/hello"

[[applications]]
file_path = "./samples/testbackend.wasm"
method = "GET"
url_path = "/backend"
```

### Running it

You can build and launch the server as follows:

```rust
cargo build && ./target/debug/serverless-wasm ./samples/config.toml
```

## Current features

- [x] load web assembly file to handle requests
- [x] logging function available from WASM
- [x] API to build a response from WASM
- [x] (blocking) TCP connections to backend servers or databases
- [x] routing to mutiple apps depending on the request
- [x] set up initial state via "environment variables"
- [ ] proper error handling (the server will panic even if you give it the side eye)
- [ ] (in progress) asynchronous event loop to receive connections and handle backend TCP connections
- [ ] file system abstraction (loading files from S3 or other providers?)
- [ ] (in progress) "standard API" for functions exported by the VM

## Prior art

While I was building this, I heard of [IceCore](https://github.com/losfair/IceCore),
which looks quite cool, with JIT support, etc.
It's quite nice to see multiple platforms attempting this. Maybe we'll be able to
agree onthe "web assembly standard API" so WASM apps can run on any of those :)
