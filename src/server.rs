use jsonrpc_core::futures;
use jsonrpc_core::futures::future::{Future, FutureResult};
use jsonrpc_core::{Error, IoHandler, Params};
use lsp_types::Url;
use lsp_types::{DidOpenTextDocumentParams, TextDocumentItem};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use sv_parser::SyntaxTree;
use trie_rs::trie::Trie;

pub struct LSPServer {
    srcs: Vec<TextDocumentItem>,
}

struct Document {
    name: Url,
    language_id: String,
    version: i64,
    text: String,
    stree: SyntaxTree,
    ctree: Trie<u8>,
}

impl LSPServer {
    pub fn new() -> LSPServer {
        LSPServer { srcs: Vec::new() }
    }

    fn did_open(&mut self, params: Params) {
        let document: TextDocumentItem = params
            .parse::<DidOpenTextDocumentParams>()
            .unwrap()
            .text_document;
        self.srcs.push(document);
    }

    fn say_hello(&self, _: Params) -> FutureResult<Value, Error> {
        futures::finished(Value::String("hello, world".to_owned()))
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

    notification!("didOpen", did_open, server, io);
    request!("say_hello", say_hello, server, io);

    let request =
        r#"{"jsonrpc": "2.0", "method": "say_hello", "params": { "name": "world" }, "id": 1}"#;
    let response = r#"{"jsonrpc":"2.0","result":"hello, world","id":1}"#;

    assert_eq!(
        io.handle_request(request).wait().unwrap(),
        Some(response.to_owned())
    );

    io
}
