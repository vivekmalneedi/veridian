use crate::definition::def_types::*;
use crate::definition::get_scopes;
use crate::diagnostics::{get_diagnostics, is_hidden};
use crate::server::LSPServer;
use log::{debug, error, trace};
use pathdiff::diff_paths;
use ropey::{Rope, RopeSlice};
use std::cmp::min;
use std::collections::HashMap;
use std::env::current_dir;
use std::fs;
use std::ops::Range as StdRange;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread;
use std::time::Instant;
use sv_parser::*;
use thread::JoinHandle;
use tower_lsp::lsp_types::*;
use walkdir::WalkDir;

impl LSPServer {
    pub fn did_open(&self, params: DidOpenTextDocumentParams) -> PublishDiagnosticsParams {
        let document: TextDocumentItem = params.text_document;
        let uri = document.uri.clone();
        debug!("did_open: {}", &uri);
        // check if doc is already added
        if self.srcs.names.read().unwrap().contains_key(&document.uri) {
            // convert to a did_change that replace the entire text
            self.did_change(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier::new(document.uri, document.version),
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: document.text,
                }],
            });
        } else {
            self.srcs.add(document);
        }
        // diagnostics
        let urls = self.srcs.names.read().unwrap().keys().cloned().collect();
        let file_id = self.srcs.get_id(&uri);
        let file = self.srcs.get_file(file_id).unwrap();
        let file = file.read().unwrap();
        get_diagnostics(uri, &file.text, urls, &self.conf.read().unwrap())
    }

    pub fn did_change(&self, params: DidChangeTextDocumentParams) {
        debug!("did_change: {}", &params.text_document.uri);
        let file_id = self.srcs.get_id(&params.text_document.uri);
        let file = self.srcs.get_file(file_id).unwrap();
        let mut file = file.write().unwrap();
        // loop through changes and apply
        for change in params.content_changes {
            if change.range.is_none() {
                file.text = Rope::from_str(&change.text);
            } else {
                file.text.apply_change(&change);
            }
            file.last_change_range = change.range;
        }
        file.version = params.text_document.version;
        drop(file);

        // invalidate syntaxtree and wake parse thread
        let meta_data = self.srcs.get_meta_data(file_id).unwrap();
        let (lock, cvar) = &*meta_data.read().unwrap().valid_parse;
        let mut valid = lock.lock().unwrap();
        *valid = false;
        cvar.notify_all();
    }

    pub fn did_save(&self, params: DidSaveTextDocumentParams) -> PublishDiagnosticsParams {
        let urls = self.srcs.names.read().unwrap().keys().cloned().collect();
        let file_id = self.srcs.get_id(&params.text_document.uri);
        let file = self.srcs.get_file(file_id).unwrap();
        let file = file.read().unwrap();
        get_diagnostics(
            params.text_document.uri,
            &file.text,
            urls,
            &self.conf.read().unwrap(),
        )
    }
}

/// The Source struct holds all file specific information
pub struct Source {
    pub id: usize,
    pub uri: Url,
    pub text: Rope,
    pub version: i32,
    pub syntax_tree: Option<SyntaxTree>,
    // if there is a parse error, we can remove the last change
    pub last_change_range: Option<Range>,
}

/// file metadata, including whether or not the syntax tree is up to date
pub struct SourceMeta {
    pub id: usize,
    pub valid_parse: Arc<(Mutex<bool>, Condvar)>,
    pub parse_handle: JoinHandle<()>,
}

/// find SystemVerilog/Verilog sources recursively from opened files
fn find_src_paths(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();

    for dir in dirs {
        let walker = WalkDir::new(dir).into_iter();
        for entry in walker.filter_entry(|e| !is_hidden(e)) {
            let entry = entry.unwrap();
            if entry.file_type().is_file() && entry.path().extension().is_some() {
                let extension = entry.path().extension().unwrap();

                if extension == "sv" || extension == "svh" || extension == "v" || extension == "vh"
                {
                    let entry_path = entry.path().to_path_buf();
                    if !paths.contains(&entry_path) {
                        paths.push(entry_path);
                    }
                }
            }
        }
    }
    paths
}

/// The Sources struct manages all source files
pub struct Sources {
    // all files
    pub files: Arc<RwLock<Vec<Arc<RwLock<Source>>>>>,
    // map file urls to id
    pub names: Arc<RwLock<HashMap<Url, usize>>>,
    // file metadata
    pub meta: Arc<RwLock<Vec<Arc<RwLock<SourceMeta>>>>>,
    // all source files are indexed into this tree, which can then
    // be used for completion, name resolution
    pub scope_tree: Arc<RwLock<Option<GenericScope>>>,
    // include directories, passed to parser to resolve `include
    pub include_dirs: Arc<RwLock<Vec<PathBuf>>>,
    // source directories
    pub source_dirs: Arc<RwLock<Vec<PathBuf>>>,
}

impl std::default::Default for Sources {
    fn default() -> Self {
        Self::new()
    }
}

impl Sources {
    pub fn new() -> Self {
        Self {
            files: Arc::new(RwLock::new(Vec::new())),
            names: Arc::new(RwLock::new(HashMap::new())),
            meta: Arc::new(RwLock::new(Vec::new())),
            scope_tree: Arc::new(RwLock::new(None)),
            include_dirs: Arc::new(RwLock::new(Vec::new())),
            source_dirs: Arc::new(RwLock::new(Vec::new())),
        }
    }
    pub fn init(&self) {
        let mut paths: Vec<PathBuf> = Vec::new();
        for path in &*self.include_dirs.read().unwrap() {
            paths.push(path.clone());
        }
        for path in &*self.source_dirs.read().unwrap() {
            paths.push(path.clone());
        }
        // find and add all source/header files recursively from configured include and source directories
        let src_paths = find_src_paths(&paths);
        for path in src_paths {
            if let Ok(url) = Url::from_file_path(&path) {
                if let Ok(text) = fs::read_to_string(&path) {
                    self.add(TextDocumentItem::new(
                        url,
                        "systemverilog".to_string(),
                        -1,
                        text,
                    ));
                }
            }
        }
    }

    /// add a source file, creating a parse thread for that file
    pub fn add(&self, doc: TextDocumentItem) {
        // use a condvar to synchronize the parse thread
        // the valid bool decides whether or not the file
        // needs to be re-parsed
        #[allow(clippy::mutex_atomic)] // https://github.com/rust-lang/rust-clippy/issues/1516
        let valid_parse = Arc::new((Mutex::new(false), Condvar::new()));
        let valid_parse2 = valid_parse.clone();
        let mut files = self.files.write().unwrap();
        let source = Arc::new(RwLock::new(Source {
            id: files.len(),
            uri: doc.uri.clone(),
            text: Rope::from_str(&doc.text),
            version: doc.version,
            syntax_tree: None,
            last_change_range: None,
        }));
        let source_handle = source.clone();
        let scope_handle = self.scope_tree.clone();
        let inc_dirs = self.include_dirs.clone();

        // spawn parse thread
        let parse_handle = thread::spawn(move || {
            let (lock, cvar) = &*valid_parse2;
            loop {
                let now = Instant::now();
                let file = source_handle.read().unwrap();
                let text = file.text.clone();
                let uri = &file.uri.clone();
                let range = &file.last_change_range.clone();
                drop(file);
                trace!("{}, parse read: {}", uri, now.elapsed().as_millis());
                let syntax_tree = parse(&text, uri, range, &inc_dirs.read().unwrap());
                let mut scope_tree = match &syntax_tree {
                    Some(tree) => get_scopes(tree, uri),
                    None => None,
                };
                trace!(
                    "{}, parse read complete: {}",
                    uri,
                    now.elapsed().as_millis()
                );
                let mut file = source_handle.write().unwrap();
                trace!("{}, parse write: {}", uri, now.elapsed().as_millis());
                file.syntax_tree = syntax_tree;
                drop(file);
                debug!("try write global scope");
                let mut global_scope = scope_handle.write().unwrap();
                match &mut *global_scope {
                    Some(scope) => match &mut scope_tree {
                        Some(tree) => {
                            scope.defs.retain(|x| &x.url() != uri);
                            scope.scopes.retain(|x| &x.url() != uri);
                            scope.defs.append(&mut tree.defs);
                            scope.scopes.append(&mut tree.scopes);
                        }
                        None => (),
                    },
                    None => *global_scope = scope_tree,
                }
                // eprintln!("{:#?}", *global_scope);
                drop(global_scope);
                trace!("{}, write global scope", uri);
                trace!(
                    "{}, parse write complete: {}",
                    uri,
                    now.elapsed().as_millis()
                );
                let mut valid = lock.lock().unwrap();
                *valid = true;
                cvar.notify_all();
                while *valid {
                    valid = cvar.wait(valid).unwrap();
                }
            }
        });
        files.push(source);
        let fid = files.len() - 1;
        self.meta
            .write()
            .unwrap()
            .push(Arc::new(RwLock::new(SourceMeta {
                id: fid,
                valid_parse,
                parse_handle,
            })));
        debug!("added {}", &doc.uri);
        self.names.write().unwrap().insert(doc.uri, fid);
    }

    /// get file by id
    pub fn get_file(&self, id: usize) -> Option<Arc<RwLock<Source>>> {
        let files = self.files.read().ok()?;
        for file in files.iter() {
            let source = file.read().ok()?;
            if source.id == id {
                return Some(file.clone());
            }
        }
        None
    }

    /// get metadata by file id
    pub fn get_meta_data(&self, id: usize) -> Option<Arc<RwLock<SourceMeta>>> {
        let meta = self.meta.read().ok()?;
        for data in meta.iter() {
            let i = data.read().ok()?;
            if i.id == id {
                return Some(data.clone());
            }
        }
        None
    }

    /// wait for a valid parse
    pub fn wait_parse_ready(&self, id: usize, wait_valid: bool) {
        let file = self.get_file(id).unwrap();
        let file = file.read().unwrap();
        if file.syntax_tree.is_none() || wait_valid {
            drop(file);
            let meta_data = self.get_meta_data(id).unwrap();
            let (lock, cvar) = &*meta_data.read().unwrap().valid_parse;
            let mut valid = lock.lock().unwrap();
            while !*valid {
                valid = cvar.wait(valid).unwrap();
            }
        }
    }

    /// get file id from url
    pub fn get_id(&self, uri: &Url) -> usize {
        *self.names.read().unwrap().get(uri).unwrap()
    }

    /// compute identifier completions
    pub fn get_completions(
        &self,
        token: &str,
        byte_idx: usize,
        url: &Url,
    ) -> Option<CompletionList> {
        debug!("retrieving identifier completion for token: {}", &token);
        Some(CompletionList {
            is_incomplete: false,
            items: self
                .scope_tree
                .read()
                .ok()?
                .as_ref()?
                .get_completion(token, byte_idx, url),
        })
    }

    /// compute dot completions
    pub fn get_dot_completions(
        &self,
        token: &str,
        byte_idx: usize,
        url: &Url,
    ) -> Option<CompletionList> {
        debug!("retrieving dot completion for token: {}", &token);
        let tree = self.scope_tree.read().ok()?;
        Some(CompletionList {
            is_incomplete: false,
            items: tree
                .as_ref()?
                .get_dot_completion(token, byte_idx, url, tree.as_ref()?),
        })
    }
}

//TODO: show all unrecoverable parse errors to user
/// parse the file using sv-parser, attempt to recover if the parse fails
pub fn parse(
    doc: &Rope,
    uri: &Url,
    last_change_range: &Option<Range>,
    inc_paths: &[PathBuf],
) -> Option<SyntaxTree> {
    let mut parse_iterations = 1;
    let mut i = 0;
    let mut includes: Vec<PathBuf> = inc_paths.to_vec();
    let mut reverted_change = false;
    let mut text = doc.clone();

    while i < parse_iterations {
        i += 1;
        match parse_sv_str(
            &text.to_string(),
            uri.to_file_path().unwrap(),
            &HashMap::new(),
            &includes,
            false,
            true,
        ) {
            Ok((syntax_tree, _)) => {
                debug!("parse complete of {}", uri);
                trace!("{}", syntax_tree.to_string());
                return Some(syntax_tree);
            }
            Err(err) => {
                match err {
                    // syntax error
                    sv_parser::Error::Parse(trace) => match trace {
                        Some((_, bpos)) => {
                            let mut line_start = text.byte_to_line(bpos);
                            let mut line_end = text.byte_to_line(bpos) + 1;
                            if !reverted_change {
                                if let Some(range) = last_change_range {
                                    line_start = range.start.line as usize;
                                    line_end = range.end.line as usize + 1;
                                    reverted_change = true;
                                }
                            }
                            for line_idx in line_start..line_end {
                                let line = text.line(line_idx);
                                let start_char = text.line_to_char(line_idx);
                                let line_length = line.len_chars();
                                text.remove(start_char..(start_char + line_length - 1));
                                text.insert(start_char, &" ".to_owned().repeat(line_length));
                            }
                            parse_iterations += 1;
                        }
                        None => return None,
                    },
                    // include error, take the include path from the error message and
                    // add it as an include dir for the next parser invocation
                    sv_parser::Error::Include { source: x } => {
                        if let sv_parser::Error::File { source: _, path: z } = *x {
                            // Include paths have to be relative to the working directory
                            // so we have to convert a source file relative path to a working directory
                            // relative path. This should have been handled by sv-parser
                            let mut inc_path_given = z.clone();
                            let mut uri_path = uri.to_file_path().unwrap();
                            uri_path.pop();
                            let rel_path = diff_paths(uri_path, current_dir().unwrap()).unwrap();
                            inc_path_given.pop();
                            let inc_path = rel_path.join(inc_path_given);
                            if !includes.contains(&inc_path) {
                                includes.push(inc_path);
                            } else {
                                error!("parser: include error: {:?}", z);
                                break;
                            }
                            parse_iterations += 1;
                        }
                    }
                    _ => error!("parse error, {:?}", err),
                };
            }
        }
    }
    None
}

//TODO: add bounds checking for utf8<->utf16 conversions
/// This trait defines some helper functions to convert between lsp types
/// and char/byte positions
pub trait LSPSupport {
    fn pos_to_byte(&self, pos: &Position) -> usize;
    fn pos_to_char(&self, pos: &Position) -> usize;
    fn byte_to_pos(&self, byte_idx: usize) -> Position;
    fn char_to_pos(&self, char_idx: usize) -> Position;
    fn range_to_char_range(&self, range: &Range) -> StdRange<usize>;
    fn char_range_to_range(&self, range: StdRange<usize>) -> Range;
    fn apply_change(&mut self, change: &TextDocumentContentChangeEvent);
}

/// Extend ropey's Rope type with lsp convenience functions
impl LSPSupport for Rope {
    fn pos_to_byte(&self, pos: &Position) -> usize {
        self.char_to_byte(self.pos_to_char(pos))
    }
    fn pos_to_char(&self, pos: &Position) -> usize {
        let line_slice = self.line(pos.line as usize);
        self.line_to_char(pos.line as usize) + line_slice.utf16_cu_to_char(pos.character as usize)
    }
    fn byte_to_pos(&self, byte_idx: usize) -> Position {
        self.char_to_pos(self.byte_to_char(min(byte_idx, self.len_bytes() - 1)))
    }
    fn char_to_pos(&self, char_idx: usize) -> Position {
        let line = self.char_to_line(char_idx);
        let line_slice = self.line(line);
        Position {
            line: line as u32,
            character: line_slice.char_to_utf16_cu(char_idx - self.line_to_char(line)) as u32,
        }
    }
    fn range_to_char_range(&self, range: &Range) -> StdRange<usize> {
        self.pos_to_char(&range.start)..self.pos_to_char(&range.end)
    }
    fn char_range_to_range(&self, range: StdRange<usize>) -> Range {
        Range {
            start: self.char_to_pos(range.start),
            end: self.char_to_pos(range.end),
        }
    }
    fn apply_change(&mut self, change: &TextDocumentContentChangeEvent) {
        if let Some(range) = change.range {
            let char_range = self.range_to_char_range(&range);
            self.remove(char_range.clone());
            if !change.text.is_empty() {
                self.insert(char_range.start, &change.text);
            }
        }
    }
}

impl<'a> LSPSupport for RopeSlice<'a> {
    fn pos_to_byte(&self, pos: &Position) -> usize {
        self.char_to_byte(self.pos_to_char(pos))
    }
    fn pos_to_char(&self, pos: &Position) -> usize {
        let line_slice = self.line(pos.line as usize);
        self.line_to_char(pos.line as usize) + line_slice.utf16_cu_to_char(pos.character as usize)
    }
    fn byte_to_pos(&self, byte_idx: usize) -> Position {
        self.char_to_pos(self.byte_to_char(min(byte_idx, self.len_bytes() - 1)))
    }
    fn char_to_pos(&self, char_idx: usize) -> Position {
        let line = self.char_to_line(char_idx);
        let line_slice = self.line(line);
        Position {
            line: line as u32,
            character: line_slice.char_to_utf16_cu(char_idx - self.line_to_char(line)) as u32,
        }
    }
    fn range_to_char_range(&self, range: &Range) -> StdRange<usize> {
        self.pos_to_char(&range.start)..self.pos_to_char(&range.end)
    }
    fn char_range_to_range(&self, range: StdRange<usize>) -> Range {
        Range {
            start: self.char_to_pos(range.start),
            end: self.char_to_pos(range.end),
        }
    }
    fn apply_change(&mut self, _: &TextDocumentContentChangeEvent) {
        panic!("can't edit a rope slice");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::test_init;
    use std::fs::read_to_string;

    #[test]
    fn test_open_and_change() {
        test_init();
        let server = LSPServer::new(None);
        let uri = Url::parse("file:///test.sv").unwrap();
        let text = r#"module test;
  logic abc;
endmodule"#;

        let open_params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "systemverilog".to_owned(),
                version: 0,
                text: text.to_owned(),
            },
        };
        server.did_open(open_params);
        let fid = server.srcs.get_id(&uri);
        let file = server.srcs.get_file(fid).unwrap();
        let file = file.read().unwrap();
        assert_eq!(file.text.to_string(), text.to_owned());
        drop(file);

        let change_params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version: 1 },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: Some(Range {
                    start: Position {
                        line: 1,
                        character: 8,
                    },
                    end: Position {
                        line: 1,
                        character: 11,
                    },
                }),
                range_length: None,
                text: "var1".to_owned(),
            }],
        };
        server.did_change(change_params);
        let file = server.srcs.get_file(fid).unwrap();
        let file = file.read().unwrap();
        assert_eq!(
            file.text.to_string(),
            r#"module test;
  logic var1;
endmodule"#
                .to_owned()
        );
        assert_eq!(file.version, 1);
    }

    #[test]
    fn test_fault_tolerance() {
        test_init();
        let server = LSPServer::new(None);
        let uri = Url::parse("file:///test.sv").unwrap();
        let text = r#"module test;
  logic abc
endmodule"#;
        let open_params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "systemverilog".to_owned(),
                version: 0,
                text: text.to_owned(),
            },
        };
        server.did_open(open_params);
        let fid = server.srcs.get_id(&uri);

        server.srcs.wait_parse_ready(fid, true);

        assert!(server
            .srcs
            .scope_tree
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .contains_scope("test"));
    }

    #[test]
    fn test_header() {
        test_init();
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("test_data/top_inc.sv");
        let text = read_to_string(&d).unwrap();
        let doc = Rope::from_str(&text);
        assert!(parse(&doc, &Url::from_file_path(d).unwrap(), &None, &Vec::new()).is_some(),);
        // TODO: add missing header test
    }
}
