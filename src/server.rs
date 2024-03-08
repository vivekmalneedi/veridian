use crate::sources::*;

use crate::completion::keyword::*;
use flexi_logger::LoggerHandle;
use log::{debug, info, warn};
use path_clean::PathClean;
use serde::{Deserialize, Serialize};
use std::env::current_dir;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::string::ToString;
use std::sync::{Mutex, RwLock};
use tower_lsp::jsonrpc::{Error, ErrorCode, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use which::which;

pub struct LSPServer {
    pub srcs: Sources,
    pub key_comps: Vec<CompletionItem>,
    pub sys_tasks: Vec<CompletionItem>,
    pub directives: Vec<CompletionItem>,
    pub conf: RwLock<ProjectConfig>,
    pub log_handle: Mutex<Option<LoggerHandle>>,
}

impl LSPServer {
    pub fn new(log_handle: Option<LoggerHandle>) -> LSPServer {
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
    pub fn new(client: Client, log_handle: LoggerHandle) -> Backend {
        Backend {
            client,
            server: LSPServer::new(Some(log_handle)),
        }
    }
}

#[derive(strum_macros::Display, Debug, Serialize, Deserialize)]
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
#[serde(default)]
pub struct ProjectConfig {
    // if true, recursively search the working directory for files to run diagnostics on
    pub auto_search_workdir: bool,
    // list of directories with header files
    pub include_dirs: Vec<String>,
    // list of directories to recursively search for SystemVerilog/Verilog sources
    pub source_dirs: Vec<String>,
    // config options for verible tools
    pub verible: Verible,
    // config options for verilator tools
    pub verilator: Verilator,
    // log level
    pub log_level: LogLevel,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        ProjectConfig {
            auto_search_workdir: true,
            include_dirs: Vec::new(),
            source_dirs: Vec::new(),
            verible: Verible::default(),
            verilator: Verilator::default(),
            log_level: LogLevel::Info,
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Verible {
    pub syntax: VeribleSyntax,
    pub format: VeribleFormat,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct VeribleSyntax {
    pub enabled: bool,
    pub path: String,
    pub args: Vec<String>,
}

impl Default for VeribleSyntax {
    fn default() -> Self {
        Self {
            enabled: true,
            path: "verible-verilog-syntax".to_string(),
            args: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Verilator {
    pub syntax: VerilatorSyntax,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct VerilatorSyntax {
    pub enabled: bool,
    pub path: String,
    pub args: Vec<String>,
}

impl Default for VerilatorSyntax {
    fn default() -> Self {
        Self {
            enabled: true,
            path: "verilator".to_string(),
            args: vec![
                "--lint-only".to_string(),
                "--sv".to_string(),
                "-Wall".to_string(),
            ],
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct VeribleFormat {
    pub enabled: bool,
    pub path: String,
    pub args: Vec<String>,
}

impl Default for VeribleFormat {
    fn default() -> Self {
        Self {
            enabled: true,
            path: "verible-verilog-format".to_string(),
            args: Vec::new(),
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
        match read_config(params.root_uri) {
            Ok(conf) => {
                inc_dirs.extend(conf.include_dirs.iter().filter_map(|x| absolute_path(x)));
                debug!("{:#?}", inc_dirs);
                src_dirs.extend(conf.source_dirs.iter().filter_map(|x| absolute_path(x)));
                debug!("{:#?}", src_dirs);
                let mut log_handle = self.server.log_handle.lock().unwrap();
                let log_handle = log_handle.as_mut();
                if let Some(handle) = log_handle {
                    handle
                        .parse_and_push_temp_spec(&conf.log_level.to_string())
                        .map_err(|e| Error {
                            code: ErrorCode::InvalidParams,
                            message: e.to_string().into(),
                            data: None,
                        })?;
                }
                *self.server.conf.write().unwrap() = conf;
            }
            Err(e) => {
                warn!("found errors in config file: {:#?}", e);
            }
        }
        let mut conf = self.server.conf.write().unwrap();
        conf.verible.syntax.enabled = which(&conf.verible.syntax.path).is_ok();
        if cfg!(feature = "slang") {
            info!("enabled linting with slang");
        }
        if conf.verilator.syntax.enabled {
            info!("enabled linting with verilator")
        } else if conf.verible.syntax.enabled {
            info!("enabled linting with verible-verilog-syntax")
        }
        conf.verible.format.enabled = which(&conf.verible.format.path).is_ok();
        if conf.verible.format.enabled {
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
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
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
                    all_commit_characters: None,
                    //TODO: check if corect
                    completion_item: None,
                }),
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(conf.verible.format.enabled)),
                document_range_formatting_provider: Some(OneOf::Left(conf.verible.format.enabled)),
                document_highlight_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
        })
    }
    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "veridian initialized!")
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
    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        Ok(self.server.document_highlight(params))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config() {
        let config = r#"
auto_search_workdir: false
format: true
verible:
  syntax:
    enabled: true
    path: "verible-verilog-syntax"
  format:
    args:
      - --net_variable_alignment=align
log_level: Info
"#;
        let config = serde_yaml::from_str::<ProjectConfig>(config);
        dbg!(&config);
        assert!(config.is_ok());
    }
}
