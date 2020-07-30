use crate::server::LSPServer;
use crate::sources::{FileData, Scope};
use codespan::ByteIndex;
use codespan::LineIndex;
use codespan_lsp::position_to_byte_index;
use jsonrpc_core::futures;
use jsonrpc_core::futures::future::FutureResult;
use jsonrpc_core::{Error, Params, Value};
use lsp_types::*;
use serde_json::to_string;
use std::str;
use sv_parser::*;
use trie_rs::{Trie, TrieBuilder};

impl LSPServer {
    pub fn completion(&self, params: Params) -> FutureResult<Value, Error> {
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

fn get_identifiers(syntax_tree: &SyntaxTree) -> Vec<(String, ByteIndex)> {
    let mut idents: Vec<(String, ByteIndex)> = Vec::new();
    for node in syntax_tree {
        match node {
            RefNode::Identifier(x) => {
                let id = match x {
                    Identifier::SimpleIdentifier(x) => x.nodes.0,
                    Identifier::EscapedIdentifier(x) => x.nodes.0,
                };
                let id_str = syntax_tree.get_str(&id).unwrap();
                let idb = ByteIndex(syntax_tree.get_origin(&id).unwrap().1 as u32);
                idents.push((id_str.to_owned(), idb));
            }
            _ => (),
        }
    }
    idents
}

pub fn get_scopes(syntax_tree: &SyntaxTree) -> Vec<Scope> {
    let mut scopes: Vec<Scope> = Vec::new();
    let identifiers = get_identifiers(&syntax_tree);

    fn build_trie(
        start: ByteIndex,
        end: ByteIndex,
        identifiers: &Vec<(String, ByteIndex)>,
    ) -> Trie<u8> {
        let mut builder = TrieBuilder::new();
        for id in identifiers {
            if id.1 >= start && id.1 <= end {
                builder.push(&id.0);
            }
        }
        builder.build()
    }

    for node in syntax_tree {
        match node {
            RefNode::ModuleDeclarationAnsi(x) => {
                // Declaration -> Keyword -> Locate
                let end = syntax_tree.get_origin(&x.nodes.3.nodes.0).unwrap().1;
                // Declaration -> Header -> ModuleKeyword
                let start = match &x.nodes.0.nodes.1 {
                    ModuleKeyword::Module(x) | ModuleKeyword::Macromodule(x) => x.nodes.0,
                };
                let start = syntax_tree.get_origin(&start).unwrap().1;
                // Declaration -> Header -> ModuleIdentifier -> Identifier
                let name = match &x.nodes.0.nodes.3.nodes.0 {
                    Identifier::SimpleIdentifier(x) => x.nodes.0,
                    Identifier::EscapedIdentifier(x) => x.nodes.0,
                };
                let name = syntax_tree.get_str(&name).unwrap();
                scopes.push(Scope {
                    name: name.to_owned(),
                    start: ByteIndex(start as u32),
                    end: ByteIndex(end as u32),
                    trie: build_trie(
                        ByteIndex(start as u32),
                        ByteIndex(end as u32),
                        &identifiers,
                    ),
                });
            }
            RefNode::ModuleDeclarationNonansi(x) => {
                // Declaration -> Keyword -> Locate
                let end = syntax_tree.get_origin(&x.nodes.3.nodes.0).unwrap().1;
                // Declaration -> Header -> ModuleKeyword
                let start = match &x.nodes.0.nodes.1 {
                    ModuleKeyword::Module(x) | ModuleKeyword::Macromodule(x) => x.nodes.0,
                };
                let start = syntax_tree.get_origin(&start).unwrap().1;
                // Declaration -> Header -> ModuleIdentifier -> Identifier
                let name = match &x.nodes.0.nodes.3.nodes.0 {
                    Identifier::SimpleIdentifier(x) => x.nodes.0,
                    Identifier::EscapedIdentifier(x) => x.nodes.0,
                };
                let name = syntax_tree.get_str(&name).unwrap();
                scopes.push(Scope {
                    name: name.to_owned(),
                    start: ByteIndex(start as u32),
                    end: ByteIndex(end as u32),
                    trie: build_trie(
                        ByteIndex(start as u32),
                        ByteIndex(end as u32),
                        &identifiers,
                    ),
                });
            }
            _ => (),
        }
    }
    scopes
}

pub fn get_completion(
    line: String,
    data: &FileData,
    pos: Position,
    bpos: ByteIndex,
) -> CompletionList {
    let token = get_completion_token(line.clone(), pos);
    let mut scopes: Vec<&Scope> = data
        .scopes
        .iter()
        .filter(|x| bpos >= x.start && bpos <= x.end)
        .collect();
    scopes.sort_by(|a, b| (a.end - a.start).cmp(&(b.end - b.start)));
    let scope = *scopes.get(0).unwrap();
    let results = scope.trie.predictive_search(&token);
    let results_in_str: Vec<&str> = results
        .iter()
        .map(|u8s| str::from_utf8(u8s).unwrap())
        .collect();
    CompletionList {
        is_incomplete: results_in_str.contains(&token.as_str()),
        items: results_in_str
            .iter()
            .map(|x| CompletionItem {
                label: (*x).to_owned(),
                kind: None,
                detail: None,
                documentation: None,
                deprecated: None,
                preselect: None,
                sort_text: None,
                filter_text: None,
                insert_text: None,
                insert_text_format: None,
                text_edit: None,
                additional_text_edits: None,
                command: None,
                data: None,
                tags: None,
            })
            .collect(),
    }
}

fn get_completion_token(line: String, pos: Position) -> String {
    let count = line.chars().count();
    let mut line_rev = line.chars().rev();
    for _ in 0..(count - (pos.character + 1) as usize) {
        line_rev.next();
    }
    let mut token = String::new();
    let mut c: char = line_rev.next().unwrap();
    while c.is_alphanumeric() {
        token.push(c);
        c = line_rev.next().unwrap();
    }
    token.chars().rev().collect()
}
