use jsonrpc_core::{IoHandler, Params};
use std::sync::{Arc, Mutex};

use crate::sources::*;

pub struct LSPServer {
    pub srcs: Sources,
}

impl LSPServer {
    pub fn new() -> LSPServer {
        LSPServer {
            srcs: Sources::new(),
        }
    }
}

macro_rules! notification {
    ($method:expr, $name:ident, $server:ident, $io:ident) => {
        let handle = $server.clone();
        $io.add_notification($method, move |p: Params| {
            handle.lock().unwrap().$name(p);
        });
    };
}

macro_rules! request {
    ($method:expr, $name:ident, $server:ident, $io:ident) => {
        let handle = $server.clone();
        $io.add_method($method, move |p: Params| handle.lock().unwrap().$name(p));
    };
}

pub fn init() -> IoHandler {
    let server: Arc<Mutex<LSPServer>> = Arc::new(Mutex::new(LSPServer::new()));
    let mut io = IoHandler::new();

    notification!("textDocument/didOpen", did_open, server, io);
    request!("textDocument/completion", completion, server, io);

    io
}
