#![recursion_limit = "256"]
// use jsonrpc_stdio_server::ServerBuilder;

mod completion;
mod server;
mod sources;

fn main() {
    let io = server::init();
}
