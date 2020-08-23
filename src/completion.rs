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
        let doc = params.text_document_position;
        let file_id = self.srcs.get_id(&doc.text_document.uri).to_owned();
        self.srcs.wait_parse_ready(file_id, false);
        let file = self.srcs.get_file(file_id)?;
        let file = file.read().ok()?;
        let token = get_completion_token(file.text.line(doc.position.line as usize), doc.position);
        Some(CompletionResponse::List(file.get_completions(
            &token,
            file.text.pos_to_byte(&doc.position),
        )?))
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

pub fn get_scopes(syntax_tree: &SyntaxTree, file_len: usize) -> Scope {
    let mut global_scope: Scope = Scope::new(("global".to_string(), 0, file_len));
    let identifiers = get_identifiers(&syntax_tree);
    let scope_idents = get_scope_idents(&syntax_tree);
    let defs = get_definitions(&syntax_tree, &scope_idents);
    for scope in scope_idents {
        global_scope.insert_scope(scope);
    }
    for def in defs {
        global_scope.insert_def(def);
    }
    for ident in identifiers {
        global_scope.insert_ident(ident);
    }
    global_scope.lift_nested_scope_defs();
    global_scope
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
        let fid = server.srcs.get_id(&uri);
        server.srcs.wait_parse_ready(fid, true);

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
        server.srcs.wait_parse_ready(fid, true);

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

    #[test]
    fn test_nested_completion() {
        let server = LSPServer::new();
        let uri = Url::parse("file:///test.sv").unwrap();
        let text = r#"module test;
    logic aouter;
    function func1();
        logic abc;
        func1 = abc;
    endfunction
    function func2();
        logic abcd;
        func2 = abcd;
    endfunction
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
        let fid = server.srcs.get_id(&uri);
        server.srcs.wait_parse_ready(fid, true);

        let change_params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: Some(3),
            },
            content_changes: vec![
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
        server.srcs.wait_parse_ready(fid, true);

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
        let item3 = CompletionItem {
            label: "aouter".to_owned(),
            kind: Some(CompletionItemKind::Variable),
            detail: Some("logic".to_string()),
            ..CompletionItem::default()
        };
        if let CompletionResponse::List(item) = response {
            assert!(item.items.contains(&item1));
            for comp in &item.items {
                assert!(comp.label != "abcd");
            }
            assert!(item.items.contains(&item3));
        } else {
            assert!(false);
        }
    }
}
