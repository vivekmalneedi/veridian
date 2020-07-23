use codespan::{ByteIndex, FileId, Files};
use lsp_types::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str;
use sv_parser::*;
use trie_rs::{Trie, TrieBuilder};

fn parse(doc: &str) -> Result<SyntaxTree, sv_parser::Error> {
    match parse_sv_str(doc, PathBuf::from(""), &HashMap::new(), &[""], false) {
        Ok((syntax_tree, _)) => Ok(syntax_tree),
        Err(err) => {
            eprintln!("{}", err);
            Err(err)
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

fn get_scopes(syntax_tree: &SyntaxTree) -> Vec<Scope> {
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

pub struct Sources {
    pub files: Files<String>,
    fdata: Vec<FileData>,
    names: HashMap<String, FileId>,
}

impl Sources {
    pub fn new() -> Sources {
        Sources {
            files: Files::new(),
            fdata: Vec::new(),
            names: HashMap::new(),
        }
    }
    pub fn add(&mut self, name: Url, doc: String) {
        let fid = self.files.add(name.as_str().to_owned(), doc);
        self.names.insert(name.as_str().to_owned(), fid);
        match parse(self.files.source(fid)) {
            Ok(syntax_tree) => self.fdata.push(FileData {
                id: fid,
                scopes: get_scopes(&syntax_tree),
                syntax_tree,
            }),
            Err(_) => (),
        };
    }

    pub fn get_id(&self, name: &str) -> &FileId {
        self.names.get(name).unwrap()
    }

    pub fn get_file_data(&self, id: &FileId) -> Option<&FileData> {
        for data in self.fdata.iter() {
            if data.id == id.to_owned() {
                return Some(&data);
            }
        }
        None
    }
}

pub struct FileData {
    id: FileId,
    scopes: Vec<Scope>,
    syntax_tree: SyntaxTree,
}

struct Scope {
    name: String,
    start: ByteIndex,
    end: ByteIndex,
    trie: Trie<u8>,
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
