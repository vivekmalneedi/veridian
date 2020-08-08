use crate::sources::*;

use log::info;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

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
    async fn initialize(&self, _: &Client, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::Incremental),
                        will_save: None,
                        will_save_wait_until: None,
                        save: Some(SaveOptions { include_text: None }),
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                definition_provider: Some(true),
                hover_provider: Some(true),
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
        client.publish_diagnostics(
            diagnostics.uri,
            diagnostics.diagnostics,
            diagnostics.version,
        );
    }
    async fn did_close(&self, client: &Client, params: DidCloseTextDocumentParams) {
        self.0.lock().await.did_close(params);
    }
    async fn did_change(&self, client: &Client, params: DidChangeTextDocumentParams) {
        self.0.lock().await.did_change(params);
    }
    async fn did_save(&self, client: &Client, params: DidSaveTextDocumentParams) {
        let diagnostics = self.0.lock().await.did_save(params);
        client.publish_diagnostics(
            diagnostics.uri,
            diagnostics.diagnostics,
            diagnostics.version,
        );
    }
    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        info!("{:?}", params);
        Ok(self.0.lock().await.completion(params))
    }
    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let definition = self.0.lock().await.goto_definition(params);
        Ok(definition)
    }
    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let hover = self.0.lock().await.hover(params);
        Ok(hover)
    }
}
