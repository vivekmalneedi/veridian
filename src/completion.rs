use crate::definition::{get_definitions, Definition};
use crate::server::LSPServer;
use crate::sources::{LSPSupport, Scope};
use log::info;
use ropey::RopeSlice;
use std::collections::HashSet;
use std::str;
use sv_parser::*;
use tower_lsp::lsp_types::*;

impl LSPServer {
    pub fn completion(&self, params: CompletionParams) -> Option<CompletionResponse> {
        eprintln!("start completion");
        let doc = params.text_document_position;
        let file_id = self.srcs.get_id(&doc.text_document.uri).to_owned();
        let file = self.srcs.get_file(file_id)?;
        eprintln!("comp: getting file");
        let file = file.read().ok()?;
        eprintln!("comp: read locked file");
        Some(CompletionResponse::List(get_completion(
            file.text.line(doc.position.line as usize),
            &file.scopes,
            doc.position,
            file.text.pos_to_byte(&doc.position),
        )))
    }
}

macro_rules! scope_declaration {
    ($tree:ident, $dec:ident, $scopes:ident, ($($a:tt),*), ($($b:tt),*), ($($c:tt),*)) => {
        let start = $tree.get_origin(&$dec$(.nodes.$a)*.nodes.0).unwrap().1;
        let end = $tree.get_origin(&$dec$(.nodes.$b)*.nodes.0).unwrap().1;
        let name = match &$dec$(.nodes.$c)*.nodes.0 {
            Identifier::EscapedIdentifier(id) => $tree.get_str(&id.nodes.0).unwrap().to_owned(),
            Identifier::SimpleIdentifier(id) => $tree.get_str(&id.nodes.0).unwrap().to_owned(),
        };
        $scopes.push((name, start, end));
    };
}

// same as scope_declaration but handles Module/Macromodule keyword
macro_rules! scope_declaration_module {
    ($tree:ident, $dec:ident, $scopes:ident, ($($a:tt),*), ($($b:tt),*), ($($c:tt),*)) => {
        let start = match &$dec$(.nodes.$a)* {
            ModuleKeyword::Module(x) | ModuleKeyword::Macromodule(x) => x.nodes.0,
        };
        let start = $tree.get_origin(&start).unwrap().1;
        let end = $tree.get_origin(&$dec$(.nodes.$b)*.nodes.0).unwrap().1;
        let name = match &$dec$(.nodes.$c)*.nodes.0 {
            Identifier::EscapedIdentifier(id) => $tree.get_str(&id.nodes.0).unwrap().to_owned(),
            Identifier::SimpleIdentifier(id) => $tree.get_str(&id.nodes.0).unwrap().to_owned(),
        };
        $scopes.push((name, start, end));
    };
}

pub fn get_scope_idents(syntax_tree: &SyntaxTree) -> Vec<(String, usize, usize)> {
    let mut scope_idents: Vec<(String, usize, usize)> = Vec::new();
    for node in syntax_tree {
        match node {
            RefNode::ModuleDeclarationAnsi(x) => {
                scope_declaration_module!(syntax_tree, x, scope_idents, (0, 1), (3), (0, 3));
            }
            RefNode::ModuleDeclarationNonansi(x) => {
                scope_declaration_module!(syntax_tree, x, scope_idents, (0, 1), (3), (0, 3));
            }
            RefNode::ModuleDeclarationWildcard(x) => {
                scope_declaration_module!(syntax_tree, x, scope_idents, (1), (8), (3));
            }
            RefNode::UdpDeclarationAnsi(x) => {
                scope_declaration!(syntax_tree, x, scope_idents, (0, 1), (2), (0, 2));
            }
            RefNode::UdpDeclarationNonansi(x) => {
                scope_declaration!(syntax_tree, x, scope_idents, (0, 1), (4), (0, 2));
            }
            RefNode::UdpDeclarationWildcard(x) => {
                scope_declaration!(syntax_tree, x, scope_idents, (1), (7), (2));
            }
            RefNode::InterfaceDeclarationNonansi(x) => {
                scope_declaration!(syntax_tree, x, scope_idents, (0, 1), (3), (0, 3));
            }
            RefNode::InterfaceDeclarationAnsi(x) => {
                scope_declaration!(syntax_tree, x, scope_idents, (0, 1), (3), (0, 3));
            }
            RefNode::InterfaceDeclarationWildcard(x) => {
                scope_declaration!(syntax_tree, x, scope_idents, (1), (8), (3));
            }
            RefNode::ProgramDeclarationNonansi(x) => {
                scope_declaration!(syntax_tree, x, scope_idents, (0, 1), (3), (0, 3));
            }
            RefNode::ProgramDeclarationAnsi(x) => {
                scope_declaration!(syntax_tree, x, scope_idents, (0, 1), (3), (0, 3));
            }
            RefNode::ProgramDeclarationWildcard(x) => {
                scope_declaration!(syntax_tree, x, scope_idents, (1), (7), (2));
            }
            RefNode::PackageDeclaration(x) => {
                scope_declaration!(syntax_tree, x, scope_idents, (1), (7), (3));
            }
            RefNode::ConfigDeclaration(x) => {
                scope_declaration!(syntax_tree, x, scope_idents, (0), (6), (1));
            }
            RefNode::ClassDeclaration(x) => {
                scope_declaration!(syntax_tree, x, scope_idents, (1), (9), (3));
            }
            RefNode::FunctionDeclaration(x) => {
                let start = syntax_tree.get_origin(&x.nodes.0.nodes.0).unwrap().1;
                let end = match &x.nodes.2 {
                    FunctionBodyDeclaration::WithoutPort(node) => {
                        syntax_tree.get_origin(&node.nodes.6.nodes.0).unwrap().1
                    }
                    FunctionBodyDeclaration::WithPort(node) => {
                        syntax_tree.get_origin(&node.nodes.7.nodes.0).unwrap().1
                    }
                };
                let ident = match &x.nodes.2 {
                    FunctionBodyDeclaration::WithoutPort(node) => &node.nodes.2.nodes.0,
                    FunctionBodyDeclaration::WithPort(node) => &node.nodes.2.nodes.0,
                };
                let name = match ident {
                    Identifier::EscapedIdentifier(id) => {
                        syntax_tree.get_str(&id.nodes.0).unwrap().to_owned()
                    }
                    Identifier::SimpleIdentifier(id) => {
                        syntax_tree.get_str(&id.nodes.0).unwrap().to_owned()
                    }
                };
                scope_idents.push((name, start, end));
            }
            RefNode::TaskDeclaration(x) => {
                let start = syntax_tree.get_origin(&x.nodes.0.nodes.0).unwrap().1;
                let end = match &x.nodes.2 {
                    TaskBodyDeclaration::WithoutPort(node) => {
                        syntax_tree.get_origin(&node.nodes.5.nodes.0).unwrap().1
                    }
                    TaskBodyDeclaration::WithPort(node) => {
                        syntax_tree.get_origin(&node.nodes.6.nodes.0).unwrap().1
                    }
                };
                let ident = match &x.nodes.2 {
                    TaskBodyDeclaration::WithoutPort(node) => &node.nodes.1.nodes.0,
                    TaskBodyDeclaration::WithPort(node) => &node.nodes.1.nodes.0,
                };
                let name = match ident {
                    Identifier::EscapedIdentifier(id) => {
                        syntax_tree.get_str(&id.nodes.0).unwrap().to_owned()
                    }
                    Identifier::SimpleIdentifier(id) => {
                        syntax_tree.get_str(&id.nodes.0).unwrap().to_owned()
                    }
                };
                scope_idents.push((name, start, end));
            }
            _ => (),
        }
    }
    scope_idents
}

fn get_identifiers(syntax_tree: &SyntaxTree) -> Vec<(String, usize)> {
    let mut idents: Vec<(String, usize)> = Vec::new();
    for node in syntax_tree {
        match node {
            RefNode::Identifier(x) => {
                let id = match x {
                    Identifier::SimpleIdentifier(x) => x.nodes.0,
                    Identifier::EscapedIdentifier(x) => x.nodes.0,
                };
                let id_str = syntax_tree.get_str(&id).unwrap();
                let idb = syntax_tree.get_origin(&id).unwrap().1;
                idents.push((id_str.to_owned(), idb));
            }
            _ => (),
        }
    }
    idents
}

fn filter_idents(start: usize, end: usize, idents: &Vec<(String, usize)>) -> Vec<(String, usize)> {
    idents
        .iter()
        .filter(|x| (x.1 >= start) && (x.1 <= end))
        .map(|x| (x.0.to_owned(), x.1))
        .collect()
}

fn filter_defs(start: usize, end: usize, defs: &Vec<Definition>) -> Vec<Definition> {
    defs.iter()
        .filter(|x| (x.byte_idx >= start) && (x.byte_idx <= end))
        .map(|x| x.clone())
        .collect()
}

pub fn get_scopes(syntax_tree: &SyntaxTree) -> Vec<Scope> {
    let mut scopes: Vec<Scope> = Vec::new();
    let identifiers = get_identifiers(&syntax_tree);
    let scope_idents = get_scope_idents(&syntax_tree);
    eprintln!("scope idents complete");
    let defs = get_definitions(&syntax_tree, &scope_idents);
    eprintln!("defs complete");
    for scope in scope_idents {
        let mut idents: HashSet<String> = HashSet::new();
        filter_idents(scope.1, scope.2, &identifiers)
            .iter()
            .for_each(|x| {
                idents.insert(x.0.clone());
            });
        scopes.push(Scope {
            name: scope.0,
            start: scope.1,
            end: scope.2,
            idents,
            defs: filter_defs(scope.1, scope.2, &defs),
        });
    }
    scopes
}

pub fn get_completion(
    line: RopeSlice,
    scopes: &Vec<Scope>,
    pos: Position,
    bpos: usize,
) -> CompletionList {
    let token = get_completion_token(line, pos);
    let mut scopes: Vec<&Scope> = scopes
        .iter()
        .filter(|x| bpos >= x.start && bpos <= x.end)
        .collect();
    scopes.sort_by(|a, b| (a.end - a.start).cmp(&(b.end - b.start)));
    let scope = *scopes.get(0).unwrap();
    let mut results: Vec<CompletionItem> = scope
        .defs
        .iter()
        .filter(|x| x.ident.starts_with(&token))
        .map(|x| CompletionItem {
            label: x.ident.to_string(),
            kind: Some(x.kind),
            detail: Some(x.type_str.to_string()),
            ..CompletionItem::default()
        })
        .collect();
    let def_idents: Vec<&String> = results.iter().map(|x| &x.label).collect();
    let mut results_idents: Vec<CompletionItem> = scope
        .idents
        .iter()
        .filter(|x| !def_idents.contains(x))
        .map(|x| CompletionItem {
            label: x.to_owned(),
            ..CompletionItem::default()
        })
        .collect();
    results.append(&mut results_idents);
    CompletionList {
        is_incomplete: false,
        items: results,
    }
}

fn get_completion_token(line: RopeSlice, pos: Position) -> String {
    let mut token = String::new();
    let mut line_iter = line.chars();
    for _ in 0..(line.utf16_cu_to_char(pos.character as usize)) {
        line_iter.next();
    }
    let mut c = line_iter.prev();
    //TODO: make this a regex
    while !c.is_none() && (c.unwrap().is_alphanumeric() || c.unwrap() == '_') {
        token.push(c.unwrap());
        c = line_iter.prev();
    }
    token.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;
    use std::{thread, time};

    #[test]
    fn test_get_completion_token() {
        let text = Rope::from_str("abc abc.cba de_fg");
        let mut result = get_completion_token(
            text.line(0),
            Position {
                line: 0,
                character: 3,
            },
        );
        assert_eq!(&result, "abc");
        result = get_completion_token(
            text.line(0),
            Position {
                line: 0,
                character: 11,
            },
        );
        assert_eq!(&result, "cba");
        result = get_completion_token(
            text.line(0),
            Position {
                line: 0,
                character: 16,
            },
        );
        assert_eq!(&result, "de_f");
    }

    #[test]
    fn test_completion() {
        let server = LSPServer::new();
        let uri = Url::parse("file:///test.sv").unwrap();
        let text = r#"module test;
    logic abc;
    logic abcd;

endmodule
"#;
        let open_params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "systemverilog".to_owned(),
                version: 0,
                text: text.to_owned(),
            },
        };
        server.did_open(open_params);

        let change_params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: Some(3),
            },
            content_changes: vec![
                TextDocumentContentChangeEvent {
                    range: Some(Range {
                        start: Position {
                            line: 3,
                            character: 0,
                        },
                        end: Position {
                            line: 3,
                            character: 0,
                        },
                    }),
                    range_length: None,
                    text: "\n".to_owned(),
                },
                TextDocumentContentChangeEvent {
                    range: Some(Range {
                        start: Position {
                            line: 4,
                            character: 0,
                        },
                        end: Position {
                            line: 4,
                            character: 0,
                        },
                    }),
                    range_length: None,
                    text: "  ".to_owned(),
                },
                TextDocumentContentChangeEvent {
                    range: Some(Range {
                        start: Position {
                            line: 4,
                            character: 2,
                        },
                        end: Position {
                            line: 4,
                            character: 2,
                        },
                    }),
                    range_length: None,
                    text: "a".to_owned(),
                },
            ],
        };
        server.did_change(change_params);
        let sleep_time = time::Duration::from_secs(3);
        thread::sleep(sleep_time);

        let completion_params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 4,
                    character: 3,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::Invoked,
                trigger_character: None,
            }),
        };
        let response: CompletionResponse = server.completion(completion_params).unwrap();
        let item1 = CompletionItem {
            label: "abc".to_owned(),
            kind: Some(CompletionItemKind::Variable),
            detail: Some("logic".to_string()),
            ..CompletionItem::default()
        };
        let item2 = CompletionItem {
            label: "abcd".to_owned(),
            kind: Some(CompletionItemKind::Variable),
            detail: Some("logic".to_string()),
            ..CompletionItem::default()
        };
        if let CompletionResponse::List(item) = response {
            assert!(item.items.contains(&item1));
            assert!(item.items.contains(&item2));
        } else {
            assert!(false);
        }
    }
}
