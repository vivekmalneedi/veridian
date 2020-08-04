use crate::sources::*;

use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
// use tower_lsp::{Client, LanguageServer, LspService, Server};
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use log::info;

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

pub struct Backend(Mutex<LSPServer>);

impl Backend {
    pub fn new() -> Backend {
        Backend(Mutex::new(LSPServer::new()))
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        _: &Client,
        _: InitializeParams,
    ) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::Incremental),
                        will_save: None,
                        will_save_wait_until: None,
                        save: None,
                    },
                )),
                selection_range_provider: None,
                hover_provider: None,
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                ..ServerCapabilities::default()
            },
        })
    }
    async fn initialized(&self, client: &Client, _: InitializedParams) {
        client.log_message(MessageType::Info, "server initialized!");
    }
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
    async fn did_open(&self, client: &Client, params: DidOpenTextDocumentParams) {
        let diagnostics = self.0.lock().await.did_open(params);
        client.publish_diagnostics(diagnostics.uri, diagnostics.diagnostics, diagnostics.version);
    }
    async fn did_change(&self, client: &Client, params: DidChangeTextDocumentParams) {
        let diagnostics = self.0.lock().await.did_change(params);
        client.publish_diagnostics(diagnostics.uri, diagnostics.diagnostics, diagnostics.version);
    }
    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        info!("{:?}", params);
        Ok(self.0.lock().await.completion(params))
    }
}
