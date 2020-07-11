use jsonrpc_stdio_server::ServerBuilder;

mod server;
use server::init;

fn main() {
    init();
    // ServerBuilder::new(init()).build();
}
