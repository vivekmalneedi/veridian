use crate::definition::Definition;
use crate::server::LSPServer;
use crate::sources::LSPSupport;
use log::info;
use ropey::RopeSlice;
use std::collections::HashSet;
use std::str;
use std::time::Instant;
use sv_parser::*;
use tower_lsp::lsp_types::*;

pub mod keyword;
use keyword::*;

impl LSPServer {
    pub fn completion(&self, params: CompletionParams) -> Option<CompletionResponse> {
        let doc = params.text_document_position;
        let file_id = self.srcs.get_id(&doc.text_document.uri).to_owned();
        let now = Instant::now();
        self.srcs.wait_parse_ready(file_id, false);
        // eprintln!("comp wait parse: {}", now.elapsed().as_millis());
        let file = self.srcs.get_file(file_id)?;
        let file = file.read().ok()?;
        // eprintln!("comp read: {}", now.elapsed().as_millis());
        let token = get_completion_token(file.text.line(doc.position.line as usize), doc.position);
        eprintln!("token: {}", token);
        let response = match params.context {
            Some(context) => match context.trigger_kind {
                CompletionTriggerKind::TriggerCharacter => {
                    match context.trigger_character?.as_str() {
                        "." => Some(self.srcs.get_dot_completions(
                            token.trim_end_matches("."),
                            file.text.pos_to_byte(&doc.position),
                            &doc.text_document.uri,
                        )?),
                        "$" => Some(CompletionList {
                            is_incomplete: false,
                            items: self.sys_tasks.clone(),
                        }),
                        "`" => Some(CompletionList {
                            is_incomplete: false,
                            items: self.directives.clone(),
                        }),
                        _ => None,
                    }
                }
                CompletionTriggerKind::TriggerForIncompleteCompletions => None,
                CompletionTriggerKind::Invoked => {
                    let mut comps = self.srcs.get_completions(
                        &token,
                        file.text.pos_to_byte(&doc.position),
                        &doc.text_document.uri,
                    )?;
                    comps.items.extend::<Vec<CompletionItem>>(
                        self.key_comps
                            .iter()
                            .filter(|x| x.label.starts_with(&token))
                            .map(|x| x.clone())
                            .collect(),
                    );
                    Some(comps)
                }
            },
            None => {
                let mut comps = self.srcs.get_completions(
                    &token,
                    file.text.pos_to_byte(&doc.position),
                    &doc.text_document.uri,
                )?;
                comps.items.extend::<Vec<CompletionItem>>(
                    self.key_comps
                        .iter()
                        .filter(|x| x.label.starts_with(&token))
                        .map(|x| x.clone())
                        .collect(),
                );
                Some(comps)
            }
        };
        // eprintln!("comp response: {}", now.elapsed().as_millis());
        Some(CompletionResponse::List(response?))
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
    while !c.is_none()
        && (c.unwrap().is_alphanumeric()
            || c.unwrap() == '_'
            || c.unwrap() == '.'
            || c.unwrap() == '['
            || c.unwrap() == ']')
    {
        token.push(c.unwrap());
        c = line_iter.prev();
    }
    let mut result: String = token.chars().rev().collect();
    if result.contains('[') {
        let l_bracket_offset = result.find('[').unwrap_or(result.len());
        result.replace_range(l_bracket_offset.., "");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;
    use std::{thread, time};

    #[test]
    fn test_get_completion_token() {
        let text = Rope::from_str("abc abc.cba de_fg cde[4]");
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
        assert_eq!(&result, "abc.cba");
        result = get_completion_token(
            text.line(0),
            Position {
                line: 0,
                character: 16,
            },
        );
        assert_eq!(&result, "de_f");
        result = get_completion_token(
            text.line(0),
            Position {
                line: 0,
                character: 23,
            },
        );
        assert_eq!(&result, "cde");
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
            eprintln!("{:#?}", item);
            assert!(item.items.contains(&item1));
            for comp in &item.items {
                assert!(comp.label != "abcd");
            }
            assert!(item.items.contains(&item3));
        } else {
            assert!(false);
        }
    }

    #[test]
    fn test_dot_completion() {
        let server = LSPServer::new();
        let uri = Url::parse("file:///test.sv").unwrap();
        let text = r#"interface test_inter;
    wire abcd;
endinterface
module test(
    test_inter abc
);
    abc.
    test_inter.
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
        let file = server.srcs.get_file(fid).unwrap();
        let file = file.read().unwrap();
        /*
        eprintln!("{}", file.syntax_tree.as_ref().unwrap());
        eprintln!(
            "{:#?}",
            server.srcs.scope_tree.read().unwrap().as_ref().unwrap()
        );
        */

        let completion_params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 6,
                    character: 8,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::TriggerCharacter,
                trigger_character: Some(".".to_string()),
            }),
        };
        let response: CompletionResponse = server.completion(completion_params).unwrap();
        let item1 = CompletionItem {
            label: "abcd".to_owned(),
            kind: Some(CompletionItemKind::Variable),
            detail: Some("wire".to_string()),
            ..CompletionItem::default()
        };
        if let CompletionResponse::List(item) = response {
            eprintln!("{:#?}", item);
            assert!(item.items.contains(&item1));
            assert!(item.items.len() == 1);
        } else {
            assert!(false);
        }
        let completion_params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 7,
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::TriggerCharacter,
                trigger_character: Some(".".to_string()),
            }),
        };
        let response: CompletionResponse = server.completion(completion_params).unwrap();
        if let CompletionResponse::List(item) = response {
            eprintln!("{:#?}", item);
            assert!(item.items.contains(&item1));
            assert!(item.items.len() == 1);
        } else {
            assert!(false);
        }
    }
}
