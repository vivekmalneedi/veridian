use crate::server::LSPServer;
use crate::sources::{ParseData, Scope};
use codespan::ByteIndex;
use codespan::LineIndex;
use codespan_lsp::position_to_byte_index;
use lsp_server::{RequestId, Response};
use lsp_types::*;
use serde_json::to_value;
use std::str;
use sv_parser::*;
use trie_rs::{Trie, TrieBuilder};

impl LSPServer {
    pub fn completion(&self, id: RequestId, params: CompletionParams) -> Response {
        let doc = params.text_document_position;
        let file_id = self.srcs.get_id(doc.text_document.uri.as_str()).to_owned();
        let data = self.srcs.get_file_data(&file_id).unwrap();
        let span = self
            .srcs
            .files
            .line_span(file_id, LineIndex(doc.position.line as u32))
            .unwrap();
        let line = self
            .srcs
            .files
            .source_slice(file_id, span)
            .unwrap()
            .to_owned();
        Response {
            id,
            result: Some(
                to_value(get_completion(
                    line,
                    data,
                    doc.position,
                    position_to_byte_index(&self.srcs.files, file_id, &doc.position)
                        .unwrap(),
                ))
                .unwrap(),
            ),
            error: None,
        }
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
    data: &ParseData,
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
