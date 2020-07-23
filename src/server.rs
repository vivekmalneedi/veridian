use codespan::LineIndex;
use codespan_lsp::position_to_byte_index;
use jsonrpc_core::futures;
use jsonrpc_core::futures::future::FutureResult;
use jsonrpc_core::{Error, IoHandler, Params, Value};
use lsp_types::*;
use serde_json::to_string;
use std::sync::{Arc, Mutex};

use crate::completion::*;

pub struct LSPServer {
    srcs: Sources,
}

impl LSPServer {
    pub fn new() -> LSPServer {
        LSPServer {
            srcs: Sources::new(),
        }
    }

    fn did_open(&mut self, params: Params) {
        let document: TextDocumentItem = params
            .parse::<DidOpenTextDocumentParams>()
            .unwrap()
            .text_document;
        self.srcs.add(document.uri, document.text);
    }

    fn completion(&self, params: Params) -> FutureResult<Value, Error> {
        let c_params = params.parse::<CompletionParams>().unwrap();
        let doc = c_params.text_document_position;
        let id = self.srcs.get_id(doc.text_document.uri.as_str()).to_owned();
        let data = self.srcs.get_file_data(&id).unwrap();
        let span = self
            .srcs
            .files
            .line_span(id, LineIndex(doc.position.line as u32))
            .unwrap();
        let line = self.srcs.files.source_slice(id, span).unwrap().to_owned();
        futures::finished(Value::String(
            to_string(&get_completion(
                line,
                data,
                doc.position,
                position_to_byte_index(&self.srcs.files, id, &doc.position).unwrap(),
            ))
            .unwrap(),
        ))
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
