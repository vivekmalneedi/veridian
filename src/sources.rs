use crate::server::LSPServer;
use crate::symbol::*;
use log::debug;
use ropey::{Rope, RopeSlice};
use std::cmp::min;
use std::collections::HashMap;
use std::fs;
use std::ops::Range as StdRange;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use tower_lsp::lsp_types::*;
use walkdir::{DirEntry, WalkDir};

use tree_sitter::{InputEdit, Point, Query, Tree};

impl LSPServer {
    pub fn did_open(&self, params: DidOpenTextDocumentParams) {
        let document: TextDocumentItem = params.text_document;
        debug!("did_open: {}", &document.uri);
        // check if doc is already added
        let mut files = self.srcs.files.lock().unwrap();
        if files.contains_key(&document.uri) {
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
            files.insert(
                document.uri.clone(),
                Source {
                    text: Rope::from_str(&document.text),
                    version: document.version,
                },
            );
        }
        // TODO: trigger diagnostics
    }

    pub fn did_change(&self, params: DidChangeTextDocumentParams) {
        debug!("did_change: {}", &params.text_document.uri);
        let mut files = self.srcs.files.lock().unwrap();
        let file = files.get_mut(&params.text_document.uri).unwrap();
        let mut edits: Vec<InputEdit> = Vec::new();
        // loop through changes and apply
        for change in &params.content_changes {
            if change.range.is_none() {
                file.text = Rope::from_str(&change.text);
            } else if let Some(range) = change.range {
                let mut edit = InputEdit {
                    start_byte: file.text.pos_to_byte(&range.start),
                    old_end_byte: file.text.pos_to_byte(&range.end),
                    new_end_byte: file.text.pos_to_byte(&range.start) + change.text.len(),
                    start_position: Point {
                        row: range.start.line as usize,
                        column: range.start.character as usize,
                    },
                    old_end_position: Point {
                        row: range.end.line as usize,
                        column: range.end.character as usize,
                    },
                    new_end_position: Point { row: 0, column: 0 },
                };
                file.text.apply_change(change);
                let end_pos = file.text.byte_to_pos(edit.start_byte + change.text.len());
                edit.new_end_position = Point {
                    row: end_pos.line as usize,
                    column: end_pos.character as usize,
                };
                edits.push(edit);
            }
        }
        file.version = params.text_document.version;
        let srcs = self.srcs.files.clone();
        let index = self.srcs.index.clone();
        tokio::task::spawn_blocking(|| parse(params.text_document.uri, srcs, index, edits));
    }

    pub fn did_save(&self, params: DidSaveTextDocumentParams) {
        // TODO; trigger diagnostics
    }
}

/// The Source struct holds all file specific information
pub struct Source {
    pub text: Rope,
    pub version: i32,
}

pub struct Index {
    pub text: Rope,
    pub syms: Vec<Symbol>,
    tree: Tree,
}

fn parse(
    uri: Url,
    files: Arc<Mutex<HashMap<Url, Source>>>,
    index: Arc<Mutex<HashMap<Url, Index>>>,
    edits: Vec<InputEdit>,
) {
    let files = files.lock().unwrap();
    let mut index = index.lock().unwrap();
    let file = files.get(&uri).expect("file not in files map");
    let text = file.text.clone();
    drop(files);
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_verilog::LANGUAGE.into())
        .expect("Error loading Verilog parser");
    let query = &Query::new(&tree_sitter_verilog::LANGUAGE.into(), SYMBOL_QUERY).unwrap();

    #[allow(clippy::map_entry)]
    if index.contains_key(&uri) {
        let index = index.get_mut(&uri).unwrap();
        let tree = {
            // apply edits to tree
            for edit in edits {
                index.tree.edit(&edit);
            }
            parser.parse_with(
                &mut |offset: usize, _pos: Point| {
                    let (chunk, chunk_byte_idx, _, _) = text.chunk_at_byte(offset);
                    &chunk.as_bytes()[(offset - chunk_byte_idx)..]
                },
                Some(&index.tree),
            )
        };
        if let Some(tree) = tree {
            index.tree = tree;
            index.text = text;
            index.syms = index_text(&index.text, &index.tree, query);
        }
    } else {
        let tree = parser.parse_with(
            &mut |offset: usize, _pos: Point| {
                let (chunk, chunk_byte_idx, _, _) = text.chunk_at_byte(offset);
                &chunk.as_bytes()[(offset - chunk_byte_idx)..]
            },
            None,
        );
        if let Some(tree) = tree {
            let syms = index_text(&text, &tree, query);
            index.insert(uri, Index { text, tree, syms });
        }
    };
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
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
    pub files: Arc<Mutex<HashMap<Url, Source>>>,
    pub index: Arc<Mutex<HashMap<Url, Index>>>,
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
            files: Arc::new(Mutex::new(HashMap::new())),
            index: Arc::new(Mutex::new(HashMap::new())),
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
                    let mut files = self.files.lock().unwrap();
                    files.insert(
                        url.clone(),
                        Source {
                            text: Rope::from_str(&text),
                            version: -1,
                        },
                    );
                }
            }
        }
    }

    /// compute identifier completions
    pub fn get_completions(&self, token: &str, byte_idx: usize, uri: &Url) -> Vec<Symbol> {
        // TODO: get completions
        debug!("retrieving identifier completion for token: {}", &token);
        let index = self.index.lock().unwrap();
        let index = index.get(uri).unwrap();
        let mut cand: Vec<Symbol> = Vec::new();
        let mut stack: Vec<Symbol> = Vec::new();
        for sym in &index.syms {
            // pop scope from stack if it doesn't contain sym
            if let Some(scope) = stack.last().and_then(|s| s.scope_node) {
                if !scope.contains(sym.ident_node.start) {
                    stack.pop();
                }
            }
            // push scope to stack
            if let Some(scope) = sym.scope_node {
                // multiple definitions can create equivalent scopes
                if let Some(last) = stack.last().and_then(|s| s.scope_node) {
                    if scope != last {
                        stack.push(*sym);
                    }
                } else {
                    stack.push(*sym);
                }
            }
            // check if parent of sym contains pos
            if sym.parent.is_some() {
                if let Some(scope) = stack.last().and_then(|s| s.scope_node) {
                    if !scope.contains(byte_idx) {
                        continue;
                    }
                }
            }
            // print!("stack: ");
            // for sym in &stack {
            //     print!("{} ", range_text(sym.ident_node, text));
            // }
            // println!();

            // check if symbol identifier starts with token
            let mut text = index.text.byte_slice(sym.ident_node).chars();
            let mut starts_with = true;
            for ch in token.chars() {
                if let Some(ch2) = text.next() {
                    if ch2 != ch {
                        starts_with = false;
                    }
                } else {
                    starts_with = false;
                }
            }
            if starts_with {
                cand.push(*sym);
            }
        }
        cand
    }

    /// compute dot completions
    pub fn get_dot_completions(&self, token: &str, byte_idx: usize, uri: &Url) -> Vec<Symbol> {
        debug!("retrieving dot completion for token: {}", &token);
        // TODO: get dot completions
        let index = self.index.lock().unwrap();
        let index = index.get(uri).unwrap();
        let mut cand: Vec<Symbol> = Vec::new();
        let mut stack: Vec<Symbol> = Vec::new();
        for sym in &index.syms {
            // pop scope from stack if it doesn't contain sym
            if let Some(scope) = stack.last().and_then(|s| s.scope_node) {
                if !scope.contains(sym.ident_node.start) {
                    stack.pop();
                }
            }
            // push scope to stack
            if let Some(scope) = sym.scope_node {
                // multiple definitions can create equivalent scopes
                if let Some(last) = stack.last().and_then(|s| s.scope_node) {
                    if scope != last {
                        stack.push(*sym);
                    }
                } else {
                    stack.push(*sym);
                }
            }
            // check if parent scope of sym contains pos
            if let Some(parent) = sym.parent {
                if let Some(scope) = stack.last().and_then(|s| s.scope_node) {
                    if !scope.contains(byte_idx) {
                        continue;
                    }
                }

                // check if symbol parent identifier equals token
                let text = index.text.byte_slice(parent).chars();
                if text.eq(token.chars()) {
                    cand.push(*sym);
                }
            }
        }
        cand
    }
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

impl LSPSupport for RopeSlice<'_> {
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
