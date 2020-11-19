use crate::sources::*;

use crate::completion::keyword::*;
use flexi_logger::ReconfigurationHandle;
use log::{debug, info};
use path_clean::PathClean;
use serde::{Deserialize, Serialize};
use std::env::current_dir;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::string::ToString;
use std::sync::{Mutex, RwLock};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use which::which;

pub struct LSPServer {
    pub srcs: Sources,
    pub key_comps: Vec<CompletionItem>,
    pub sys_tasks: Vec<CompletionItem>,
    pub directives: Vec<CompletionItem>,
    pub conf: RwLock<ProjectConfig>,
    pub log_handle: Mutex<Option<ReconfigurationHandle>>,
}

impl LSPServer {
    pub fn new(log_handle: Option<ReconfigurationHandle>) -> LSPServer {
        LSPServer {
            srcs: Sources::new(),
            key_comps: keyword_completions(KEYWORDS),
            sys_tasks: other_completions(SYS_TASKS),
            directives: other_completions(DIRECTIVES),
            conf: RwLock::new(ProjectConfig::default()),
            log_handle: Mutex::new(log_handle),
        }
    }
}

pub struct Backend {
    client: Client,
    server: LSPServer,
}

impl Backend {
    pub fn new(client: Client, log_handle: ReconfigurationHandle) -> Backend {
        Backend {
            client,
            server: LSPServer::new(Some(log_handle)),
        }
    }
}

#[derive(strum_macros::ToString, Debug, Serialize, Deserialize)]
pub enum LogLevel {
    #[strum(serialize = "error")]
    Error,
    #[strum(serialize = "warn")]
    Warn,
    #[strum(serialize = "info")]
    Info,
    #[strum(serialize = "debug")]
    Debug,
    #[strum(serialize = "trace")]
    Trace,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectConfig {
    // if true, recursively search the working directory for files to run diagnostics on
    pub auto_search_workdir: bool,
    // list of directories with header files
    pub include_dirs: Vec<String>,
    // list of directories to recursively search for SystemVerilog/Verilog sources
    pub source_dirs: Vec<String>,
    // enable formatting with verible-verilog-format
    pub format: bool,
    // enable linting with Cadence verible_syntax
    pub verible_syntax: bool,
    // path to verible-verilog-format binary, defaults to verible-verilog-format
    pub verible_format_path: String,
    // path to verible_syntax binary, defaults to verible_syntax
    pub verible_syntax_path: String,
    // log level
    pub log_level: LogLevel,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        ProjectConfig {
            auto_search_workdir: true,
            include_dirs: Vec::new(),
            source_dirs: Vec::new(),
            format: false,
            verible_syntax: false,
            verible_format_path: "verible-verilog-format".to_string(),
            verible_syntax_path: "verible_syntax".to_string(),
            log_level: LogLevel::Info,
        }
    }
}

fn read_config(root_uri: Option<Url>) -> anyhow::Result<ProjectConfig> {
    let path = root_uri
        .ok_or_else(|| anyhow::anyhow!("couldn't resolve workdir path"))?
        .to_file_path()
        .map_err(|_| anyhow::anyhow!("couldn't resolve workdir path"))?;
    let mut config: Option<PathBuf> = None;
    for dir in path.ancestors() {
        let config_path = dir.join("veridian.yaml");
        if config_path.exists() {
            info!("found config: veridian.yaml");
            config = Some(config_path);
            break;
        }
        let config_path = dir.join("veridian.yml");
        if config_path.exists() {
            info!("found config: veridian.yml");
            config = Some(config_path);
            break;
        }
    }
    let mut contents = String::new();
    File::open(config.ok_or_else(|| anyhow::anyhow!("unable to read config file"))?)?
        .read_to_string(&mut contents)?;
    info!("reading config file");
    Ok(serde_yaml::from_str(&contents)?)
}

// convert string path to absolute path
fn absolute_path(path_str: &str) -> Option<PathBuf> {
    let path = PathBuf::from(path_str);
    if !path.exists() {
        return None;
    }
    if !path.has_root() {
        Some(current_dir().unwrap().join(path).clean())
    } else {
        Some(path)
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // grab include dirs and source dirs from config, and convert to abs path
        let mut inc_dirs = self.server.srcs.include_dirs.write().unwrap();
        let mut src_dirs = self.server.srcs.source_dirs.write().unwrap();
        if let Ok(conf) = read_config(params.root_uri) {
            inc_dirs.extend(conf.include_dirs.iter().filter_map(|x| absolute_path(x)));
            debug!("{:#?}", inc_dirs);
            src_dirs.extend(conf.source_dirs.iter().filter_map(|x| absolute_path(x)));
            debug!("{:#?}", src_dirs);
            let mut log_handle = self.server.log_handle.lock().unwrap();
            let log_handle = log_handle.as_mut();
            if let Some(handle) = log_handle {
                handle.parse_and_push_temp_spec(&conf.log_level.to_string());
            }
            *self.server.conf.write().unwrap() = conf;
        } else {
            info!("no config file found");
        }
        let mut conf = self.server.conf.write().unwrap();
        conf.verible_syntax = which(&conf.verible_syntax_path).is_ok();
        if cfg!(feature = "slang") {
            info!("enabled linting with slang");
        }
        if conf.verible_syntax {
            info!("enabled linting with verible-verilog-syntax")
        }
        conf.format = which(&conf.verible_format_path).is_ok();
        if conf.format {
            info!("enabled formatting with verible-verilog-format");
        } else {
            info!("formatting unavailable");
        }
        drop(inc_dirs);
        drop(src_dirs);
        // parse all source files found from walking source dirs and include dirs
        self.server.srcs.init();
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::Incremental),
                        will_save: None,
                        will_save_wait_until: None,
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: None,
                        })),
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        "$".to_string(),
                        "`".to_string(),
                    ]),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                definition_provider: Some(true),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(true),
                document_formatting_provider: Some(conf.format),
                document_range_formatting_provider: Some(conf.format),
                ..ServerCapabilities::default()
            },
        })
    }
    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::Info, "veridian initialized!")
            .await;
    }
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let diagnostics = self.server.did_open(params);
        self.client
            .publish_diagnostics(
                diagnostics.uri,
                diagnostics.diagnostics,
                diagnostics.version,
            )
            .await;
    }
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.server.did_change(params);
    }
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let diagnostics = self.server.did_save(params);
        self.client
            .publish_diagnostics(
                diagnostics.uri,
                diagnostics.diagnostics,
                diagnostics.version,
            )
            .await;
    }
    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        Ok(self.server.completion(params))
    }
    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        Ok(self.server.goto_definition(params))
    }
    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        Ok(self.server.hover(params))
    }
    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        Ok(self.server.document_symbol(params))
    }
    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        Ok(self.server.formatting(params))
    }
    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        Ok(self.server.range_formatting(params))
    }
}
