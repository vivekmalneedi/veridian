use crate::completion::get_scopes;
use crate::diagnostics::get_diagnostics;
use crate::server::LSPServer;
use log::info;
use ropey::{Rope, RopeSlice};
use std::collections::HashMap;
use std::ops::Range as StdRange;
use std::path::PathBuf;
use sv_parser::*;
use tower_lsp::lsp_types::*;
use trie_rs::Trie;

impl LSPServer {
    pub fn did_open(
        &mut self,
        params: DidOpenTextDocumentParams,
    ) -> PublishDiagnosticsParams {
        let document: TextDocumentItem = params.text_document;
        let diagnostics = get_diagnostics(document.uri.clone());
        self.srcs.add(document);
        diagnostics
    }

    pub fn did_change(
        &mut self,
        params: DidChangeTextDocumentParams,
    ) -> PublishDiagnosticsParams {
        let file_id = self.srcs.get_id(params.text_document.uri);
        let mut file = self.srcs.get_file_mut(file_id).unwrap();
        for change in params.content_changes {
            if change.range.is_none() {
                file.text = Rope::from_str(&change.text);
            } else {
                file.text.apply_change(change);
            }
        }
        if let Some(version) = params.text_document.version {
            file.version = version;
        }
        get_diagnostics(file.uri.clone())
    }
}

pub struct Source {
    pub id: usize,
    pub uri: Url,
    pub text: Rope,
    pub version: i64,
}

pub struct Sources {
    pub files: Vec<Source>,
    pub parse_data: Vec<ParseData>,
    pub names: HashMap<Url, usize>,
}

impl Sources {
    pub fn new() -> Sources {
        Sources {
            files: Vec::new(),
            parse_data: Vec::new(),
            names: HashMap::new(),
        }
    }
    pub fn add(&mut self, doc: TextDocumentItem) {
        // let fid = self.files.add(url.as_str().to_owned(), doc);
        self.files.push(Source {
            id: self.files.len(),
            uri: doc.uri.clone(),
            text: Rope::from_str(&doc.text),
            version: doc.version,
        });
        let fid = self.files.len() - 1;
        self.names.insert(doc.uri, fid);

        match parse(&doc.text) {
            Ok(syntax_tree) => self.parse_data.push(ParseData {
                id: fid,
                scopes: get_scopes(&syntax_tree),
                syntax_tree,
            }),
            Err(_) => (),
        };
    }

    pub fn get_file(&self, id: usize) -> Option<&Source> {
        for file in self.files.iter() {
            if file.id == id {
                return Some(file);
            }
        }
        None
    }

    pub fn get_file_mut(&mut self, id: usize) -> Option<&mut Source> {
        for file in self.files.iter_mut() {
            if file.id == id {
                return Some(file);
            }
        }
        None
    }

    pub fn get_id(&self, uri: Url) -> usize {
        self.names.get(&uri).unwrap().clone()
    }

    pub fn get_parse_data(&self, id: usize) -> Option<&ParseData> {
        for data in self.parse_data.iter() {
            if data.id == id {
                return Some(data);
            }
        }
        None
    }
}

pub struct ParseData {
    pub id: usize,
    pub scopes: Vec<Scope>,
    pub syntax_tree: SyntaxTree,
}

pub struct Scope {
    pub name: String,
    pub start: usize,
    pub end: usize,
    pub trie: Trie<u8>,
}

fn parse(doc: &str) -> Result<SyntaxTree, sv_parser::Error> {
    match parse_sv_str(doc, PathBuf::from(""), &HashMap::new(), &[""], false) {
        Ok((syntax_tree, _)) => Ok(syntax_tree),
        Err(err) => {
            eprintln!("{}", err);
            Err(err)
        }
    }
}

//TODO: add bounds checking for utf8<->utf16 conversions
pub trait LSPSupport {
    fn pos_to_byte(&self, pos: Position) -> usize;
    fn pos_to_char(&self, pos: Position) -> usize;
    fn byte_to_pos(&self, byte_idx: usize) -> Position;
    fn char_to_pos(&self, char_idx: usize) -> Position;
    fn range_to_char_range(&self, range: Range) -> StdRange<usize>;
    fn char_range_to_range(&self, range: StdRange<usize>) -> Range;
    fn apply_change(&mut self, change: TextDocumentContentChangeEvent);
}

impl LSPSupport for Rope {
    fn pos_to_byte(&self, pos: Position) -> usize {
        self.char_to_byte(self.pos_to_char(pos))
    }
    fn pos_to_char(&self, pos: Position) -> usize {
        let line_slice = self.line(pos.line as usize);
        self.line_to_char(pos.line as usize)
            + line_slice.utf16_cu_to_char(pos.character as usize)
    }
    fn byte_to_pos(&self, byte_idx: usize) -> Position {
        self.char_to_pos(self.byte_to_char(byte_idx))
    }
    fn char_to_pos(&self, char_idx: usize) -> Position {
        let line = self.char_to_line(char_idx);
        let line_slice = self.line(line);
        Position {
            line: line as u64,
            character: line_slice.char_to_utf16_cu(char_idx - self.line_to_char(line))
                as u64,
        }
    }
    fn range_to_char_range(&self, range: Range) -> StdRange<usize> {
        self.pos_to_char(range.start)..self.pos_to_char(range.end)
    }
    fn char_range_to_range(&self, range: StdRange<usize>) -> Range {
        Range {
            start: self.char_to_pos(range.start),
            end: self.char_to_pos(range.end),
        }
    }
    fn apply_change(&mut self, change: TextDocumentContentChangeEvent) {
        if let Some(range) = change.range {
            let char_range = self.range_to_char_range(range);
            self.remove(char_range.clone());
            if !change.text.is_empty() {
                self.insert(char_range.start, &change.text);
            }
        }
    }
}

impl<'a> LSPSupport for RopeSlice<'a> {
    fn pos_to_byte(&self, pos: Position) -> usize {
        self.char_to_byte(self.pos_to_char(pos))
    }
    fn pos_to_char(&self, pos: Position) -> usize {
        let line_slice = self.line(pos.line as usize);
        self.line_to_char(pos.line as usize)
            + line_slice.utf16_cu_to_char(pos.character as usize)
    }
    fn byte_to_pos(&self, byte_idx: usize) -> Position {
        self.char_to_pos(self.byte_to_char(byte_idx))
    }
    fn char_to_pos(&self, char_idx: usize) -> Position {
        let line = self.char_to_line(char_idx);
        let line_slice = self.line(line);
        Position {
            line: line as u64,
            character: line_slice.char_to_utf16_cu(char_idx - self.line_to_char(line))
                as u64,
        }
    }
    fn range_to_char_range(&self, range: Range) -> StdRange<usize> {
        self.pos_to_char(range.start)..self.pos_to_char(range.end)
    }
    fn char_range_to_range(&self, range: StdRange<usize>) -> Range {
        Range {
            start: self.char_to_pos(range.start),
            end: self.char_to_pos(range.end),
        }
    }
    fn apply_change(&mut self, _: TextDocumentContentChangeEvent) {
        panic!("can't edit a rope slice");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_and_change() {
        let mut server = LSPServer::new();
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
        let fid = server.srcs.get_id(uri.clone());
        let file = server.srcs.get_file(fid).unwrap();
        assert_eq!(file.text.to_string(), text.to_owned());

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
        assert_eq!(
            file.text.to_string(),
            r#"module test;
  logic var1;
endmodule"#
                .to_owned()
        );
        assert_eq!(file.version, 1);
    }
}
