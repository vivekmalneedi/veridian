use crate::completion::get_scopes;
use crate::definition::Definition;
use crate::diagnostics::get_diagnostics;
use crate::server::LSPServer;
use log::info;
use pathdiff::diff_paths;
use ropey::{Rope, RopeSlice};
use std::collections::HashMap;
use std::collections::HashSet;
use std::env::current_dir;
use std::io;
use std::ops::Range as StdRange;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Condvar, Mutex, RwLock};
use std::{thread, time::Instant};
use sv_parser::*;
use thread::JoinHandle;
use tower_lsp::lsp_types::*;

impl LSPServer {
    pub fn did_open(&self, params: DidOpenTextDocumentParams) -> PublishDiagnosticsParams {
        let document: TextDocumentItem = params.text_document;
        // let diagnostics = get_diagnostics(document.uri.clone());
        let uri = document.uri.clone();
        self.srcs.add(document);
        // diagnostics
        PublishDiagnosticsParams {
            uri,
            diagnostics: Vec::new(),
            version: Some(1),
        }
    }

    pub fn did_close(&self, params: DidCloseTextDocumentParams) {
        let document: TextDocumentIdentifier = params.text_document;
        self.srcs.remove(document);
    }

    pub fn did_change(&self, params: DidChangeTextDocumentParams) {
        let file_id = self.srcs.get_id(&params.text_document.uri);
        let file = self.srcs.get_file(file_id).unwrap();
        let mut file = file.write().unwrap();
        eprintln!("change: write locked file");
        for change in params.content_changes {
            if change.range.is_none() {
                file.text = Rope::from_str(&change.text);
            } else {
                file.text.apply_change(&change);
            }
            file.last_change_range = change.range;
        }
        if let Some(version) = params.text_document.version {
            file.version = version;
        }
        // invalidate syntaxtree and wake parse thread
        let (lock, cvar) = &*file.valid_parse;
        let mut valid = lock.lock().unwrap();
        *valid = false;
        cvar.notify_one();
        eprintln!("end change");
    }

    pub fn did_save(&self, params: DidSaveTextDocumentParams) -> PublishDiagnosticsParams {
        get_diagnostics(params.text_document.uri)
    }
}

pub struct Scope {
    pub name: String,
    pub start: usize,
    pub end: usize,
    pub idents: HashSet<String>,
    pub defs: Vec<Definition>,
}

pub struct Source {
    pub id: usize,
    pub uri: Url,
    pub text: Rope,
    pub version: i64,
    pub scopes: Vec<Scope>,
    pub syntax_tree: Option<SyntaxTree>,
    pub valid_parse: Arc<(Mutex<bool>, Condvar)>,
    pub last_change_range: Option<Range>,
}

impl Source {
    pub fn get_scope(&self, pos: &Position) -> Option<&Scope> {
        let byte_idx = self.text.pos_to_byte(pos);
        for scope in &self.scopes {
            if scope.start <= byte_idx && byte_idx <= scope.end {
                return Some(scope);
            }
        }
        None
    }
}

pub struct Sources {
    pub files: Arc<RwLock<Vec<Arc<RwLock<Source>>>>>,
    pub names: Arc<RwLock<HashMap<Url, usize>>>,
    parse_handles: Arc<RwLock<HashMap<usize, JoinHandle<()>>>>,
}

impl Sources {
    pub fn new() -> Sources {
        Sources {
            files: Arc::new(RwLock::new(Vec::new())),
            names: Arc::new(RwLock::new(HashMap::new())),
            parse_handles: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    pub fn add(&self, doc: TextDocumentItem) {
        eprintln!("start add");
        let valid_parse = Arc::new((Mutex::new(false), Condvar::new()));
        let valid_parse2 = valid_parse.clone();
        let mut files = self.files.write().unwrap();
        let source = Arc::new(RwLock::new(Source {
            id: files.len(),
            uri: doc.uri.clone(),
            text: Rope::from_str(&doc.text),
            version: doc.version,
            scopes: Vec::new(),
            syntax_tree: None,
            valid_parse,
            last_change_range: None,
        }));
        let source_handle = source.clone();
        eprintln!("spawning thread");
        let parse_handle = thread::spawn(move || {
            eprintln!("thread spawned");
            let (lock, cvar) = &*valid_parse2;
            loop {
                eprintln!("parsing");
                let file = source_handle.read().unwrap();
                eprintln!("parse: read locked file");
                let syntax_tree = parse(&file.text, &file.uri, &file.last_change_range);
                let scopes: Vec<Scope>;
                if let Some(tree) = &syntax_tree {
                    scopes = get_scopes(tree);
                } else {
                    scopes = Vec::new();
                }
                drop(file);
                eprintln!("parse complete");
                let mut file = source_handle.write().unwrap();
                eprintln!("parse: write locked file");
                file.syntax_tree = syntax_tree;
                file.scopes = scopes;
                drop(file);
                let mut valid = lock.lock().unwrap();
                *valid = true;
                eprintln!("waiting");
                while *valid {
                    valid = cvar.wait(valid).unwrap();
                }
            }
        });
        eprintln!("complete thread spawn");
        files.push(source);
        let fid = files.len() - 1;
        self.parse_handles
            .write()
            .unwrap()
            .insert(fid, parse_handle);
        self.names.write().unwrap().insert(doc.uri.clone(), fid);
        eprintln!("complete add");
    }

    pub fn remove(&self, doc: TextDocumentIdentifier) {
        let mut files = self.files.write().unwrap();
        let fid = self.get_id(&doc.uri);
        files.retain(|x| x.read().unwrap().id != fid);
    }

    pub fn get_file(&self, id: usize) -> Option<Arc<RwLock<Source>>> {
        let files = self.files.read().unwrap();
        for file in files.iter() {
            let source = file.read().unwrap();
            if source.id == id {
                return Some(file.clone());
            }
        }
        None
    }

    pub fn get_id(&self, uri: &Url) -> usize {
        self.names.read().unwrap().get(uri).unwrap().clone()
    }
}

//TODO: show all unrecoverable parse errors to user
pub fn parse(doc: &Rope, uri: &Url, last_change_range: &Option<Range>) -> Option<SyntaxTree> {
    let mut parse_iterations = 1;
    let mut i = 0;
    let mut includes: Vec<PathBuf> = Vec::new();
    let before = Instant::now();
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
        ) {
            Ok((syntax_tree, _)) => {
                eprintln!(
                    "Elapsed time complete: {:.2?}, {} iterations",
                    before.elapsed(),
                    parse_iterations
                );
                return Some(syntax_tree);
            }
            Err(err) => {
                match err {
                    sv_parser::Error::Parse(trace) => match trace {
                        Some((_, bpos)) => {
                            let mut line_start = text.byte_to_line(bpos);
                            let mut line_end = text.byte_to_line(bpos) + 1;
                            if !reverted_change {
                                if let Some(range) = last_change_range {
                                    eprintln!("previous change");
                                    line_start = range.start.line as usize;
                                    line_end = range.end.line as usize + 1;
                                    reverted_change = true;
                                }
                            }

                            eprintln!(
                                "Elapsed time parse: {:.2?}, {} iterations",
                                before.elapsed(),
                                parse_iterations
                            );
                            for line_idx in line_start..line_end {
                                let line = text.line(line_idx);
                                let start_char = text.line_to_char(line_idx);
                                let line_length = line.len_chars();
                                text.remove(start_char..(start_char + line_length));
                                text.insert(start_char, &" ".to_owned().repeat(line_length));
                            }
                            parse_iterations += 1;
                        }
                        None => return None,
                    },

                    sv_parser::Error::Include { source: x } => {
                        match *x {
                            sv_parser::Error::File { source: y, path: z } => {
                                eprintln!("handle include error");
                                let mut inc_path_given = z.clone();
                                let mut uri_path = uri.to_file_path().unwrap();
                                uri_path.pop();
                                let rel_path =
                                    diff_paths(uri_path, current_dir().unwrap()).unwrap();
                                inc_path_given.pop();
                                let inc_path = rel_path.join(inc_path_given);
                                if !includes.contains(&inc_path) {
                                    includes.push(inc_path);
                                } else {
                                    eprintln!("File Not Found: {:?}", z);
                                    break;
                                }
                                eprintln!(
                                    "Elapsed time include: {:.2?}, {} iterations",
                                    before.elapsed(),
                                    parse_iterations
                                );
                                parse_iterations += 1;
                            }
                            _ => (),
                        };
                    }
                    _ => eprintln!("parse, {:?}", err),
                };
            }
        }
    }
    None
}

//TODO: add bounds checking for utf8<->utf16 conversions
pub trait LSPSupport {
    fn pos_to_byte(&self, pos: &Position) -> usize;
    fn pos_to_char(&self, pos: &Position) -> usize;
    fn byte_to_pos(&self, byte_idx: usize) -> Position;
    fn char_to_pos(&self, char_idx: usize) -> Position;
    fn range_to_char_range(&self, range: &Range) -> StdRange<usize>;
    fn char_range_to_range(&self, range: StdRange<usize>) -> Range;
    fn apply_change(&mut self, change: &TextDocumentContentChangeEvent);
}

impl LSPSupport for Rope {
    fn pos_to_byte(&self, pos: &Position) -> usize {
        self.char_to_byte(self.pos_to_char(pos))
    }
    fn pos_to_char(&self, pos: &Position) -> usize {
        let line_slice = self.line(pos.line as usize);
        self.line_to_char(pos.line as usize) + line_slice.utf16_cu_to_char(pos.character as usize)
    }
    fn byte_to_pos(&self, byte_idx: usize) -> Position {
        self.char_to_pos(self.byte_to_char(byte_idx))
    }
    fn char_to_pos(&self, char_idx: usize) -> Position {
        let line = self.char_to_line(char_idx);
        let line_slice = self.line(line);
        Position {
            line: line as u64,
            character: line_slice.char_to_utf16_cu(char_idx - self.line_to_char(line)) as u64,
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
        self.char_to_pos(self.byte_to_char(byte_idx))
    }
    fn char_to_pos(&self, char_idx: usize) -> Position {
        let line = self.char_to_line(char_idx);
        let line_slice = self.line(line);
        Position {
            line: line as u64,
            character: line_slice.char_to_utf16_cu(char_idx - self.line_to_char(line)) as u64,
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
    use std::fs::read_to_string;
    use std::{thread, time};

    #[test]
    fn test_open_and_change() {
        let server = LSPServer::new();
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
        eprintln!("open complete");
        let fid = server.srcs.get_id(&uri);
        let file = server.srcs.get_file(fid).unwrap();
        eprintln!("attempting to read");
        let file = file.read().unwrap();
        assert_eq!(file.text.to_string(), text.to_owned());
        drop(file);
        eprintln!("open");

        let change_params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: Some(1),
            },
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
        let server = LSPServer::new();
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
        let file = server.srcs.get_file(fid).unwrap();

        let sleep_time = time::Duration::from_secs(2);
        thread::sleep(sleep_time);

        let file = file.read().unwrap();
        assert_eq!(file.scopes.get(0).unwrap().name, "test");
    }

    #[test]
    fn test_header() {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("tests_rtl/lab6/src/fp_add.sv");
        let text = read_to_string(&d).unwrap();
        let doc = Rope::from_str(&text);
        assert!(parse(&doc.clone(), &Url::from_file_path(d).unwrap(), &None).is_some());
        // TODO: add missing header test
    }
}
