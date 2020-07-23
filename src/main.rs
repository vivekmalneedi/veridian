#![recursion_limit = "256"]
// use jsonrpc_stdio_server::ServerBuilder;

mod completion;
mod server;

fn main() {
    let io = server::init();
}
