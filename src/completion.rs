use crate::server::LSPServer;
use crate::sources::LSPSupport;
use log::{debug, trace};
use ropey::{Rope, RopeSlice};
use tower_lsp::lsp_types::*;

pub mod keyword;
use keyword::*;

impl LSPServer {
    pub fn completion(&self, params: CompletionParams) -> Option<CompletionResponse> {
        debug!("completion requested");
        trace!("{:#?}", &params);
        let doc = params.text_document_position;
        let files = self.srcs.files.lock().unwrap();
        let file = files.get(&doc.text_document.uri)?;
        let token = get_completion_token(
            &file.text,
            file.text.line(doc.position.line as usize),
            doc.position,
        );
        let response = match params.context {
            Some(context) => match context.trigger_kind {
                CompletionTriggerKind::TRIGGER_CHARACTER => {
                    debug!(
                        "trigger char completion: {}",
                        context.trigger_character.clone()?.as_str()
                    );
                    match context.trigger_character?.as_str() {
                        "." => Some(CompletionList {
                            is_incomplete: false,
                            items: self.srcs.get_dot_completions(
                                token.trim_end_matches('.'),
                                file.text.pos_to_byte(&doc.position),
                                &doc.text_document.uri,
                            ),
                        }),
                        "$" => Some(CompletionList {
                            is_incomplete: false,
                            items: other_completions(SYS_TASKS),
                        }),
                        "`" => Some(CompletionList {
                            is_incomplete: false,
                            items: other_completions(DIRECTIVES),
                        }),
                        _ => None,
                    }
                }
                CompletionTriggerKind::TRIGGER_FOR_INCOMPLETE_COMPLETIONS => None,
                CompletionTriggerKind::INVOKED => {
                    debug!("Invoked Completion");
                    let mut comps: Vec<CompletionItem> = self.srcs.get_completions(
                        &token,
                        file.text.pos_to_byte(&doc.position),
                        &doc.text_document.uri,
                    );
                    // complete keywords
                    comps.extend::<Vec<CompletionItem>>(
                        keyword_completions(KEYWORDS)
                            .iter()
                            .filter(|x| x.label.starts_with(&token))
                            .cloned()
                            .collect(),
                    );
                    Some(CompletionList {
                        is_incomplete: false,
                        items: comps,
                    })
                }
                _ => None,
            },
            None => {
                let trigger = prev_char(&file.text, &doc.position);
                dbg!(&trigger);
                dbg!();
                match trigger {
                    '.' => Some(CompletionList {
                        is_incomplete: false,
                        items: self.srcs.get_dot_completions(
                            token.trim_end_matches('.'),
                            file.text.pos_to_byte(&doc.position),
                            &doc.text_document.uri,
                        ),
                    }),
                    '$' => Some(CompletionList {
                        is_incomplete: false,
                        items: other_completions(SYS_TASKS),
                    }),
                    '`' => Some(CompletionList {
                        is_incomplete: false,
                        items: other_completions(DIRECTIVES),
                    }),
                    _ => {
                        let mut comps: Vec<CompletionItem> = self.srcs.get_completions(
                            &token,
                            file.text.pos_to_byte(&doc.position),
                            &doc.text_document.uri,
                        );
                        comps.extend::<Vec<CompletionItem>>(
                            keyword_completions(KEYWORDS)
                                .iter()
                                .filter(|x| x.label.starts_with(&token))
                                .cloned()
                                .collect(),
                        );
                        Some(CompletionList {
                            is_incomplete: false,
                            items: comps,
                        })
                    }
                }
            }
        };
        // eprintln!("comp response: {}", now.elapsed().as_millis());
        Some(CompletionResponse::List(response?))
    }
}

/// get the previous non-whitespace character
fn prev_char(text: &Rope, pos: &Position) -> char {
    let char_idx = text.pos_to_char(pos);
    if char_idx > 0 {
        for i in (0..char_idx).rev() {
            let res = text.char(i);
            if !res.is_whitespace() {
                return res;
            }
        }
        ' '
    } else {
        ' '
    }
}

/// attempt to get the token the user was trying to complete, by
/// filtering out characters unneeded for name resolution
fn get_completion_token(text: &Rope, line: RopeSlice, pos: Position) -> String {
    let mut token = String::new();
    let mut line_iter = line.chars();
    for _ in 0..(line.utf16_cu_to_char(pos.character as usize)) {
        line_iter.next();
    }
    let mut c = line_iter.prev();
    //TODO: make this a regex
    while c.is_some()
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
    if &result == "." {
        // probably a instantiation, the token should be what we're instatiating
        let mut char_iter = text.chars();
        let mut token = String::new();
        for _ in 0..text.pos_to_char(&pos) {
            char_iter.next();
        }
        let mut c = char_iter.prev();

        // go to the last semicolon
        while c.is_some() && (c.unwrap() != ';') {
            c = char_iter.prev();
        }
        // go the the start of the next symbol
        while c.is_some() && !(c.unwrap().is_alphanumeric() || c.unwrap() == '_') {
            c = char_iter.next();
        }
        // then extract the next symbol
        while c.is_some() && (c.unwrap().is_alphanumeric() || c.unwrap() == '_') {
            token.push(c.unwrap());
            c = char_iter.next();
        }
        token
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::LSPSupport;
    use crate::support::test_init;
    use ropey::Rope;

    #[test]
    fn test_get_completion_token() {
        test_init();
        let text = Rope::from_str("abc abc.cba de_fg cde[4]");
        let mut result = get_completion_token(
            &text,
            text.line(0),
            Position {
                line: 0,
                character: 3,
            },
        );
        assert_eq!(&result, "abc");
        result = get_completion_token(
            &text,
            text.line(0),
            Position {
                line: 0,
                character: 11,
            },
        );
        assert_eq!(&result, "abc.cba");
        result = get_completion_token(
            &text,
            text.line(0),
            Position {
                line: 0,
                character: 16,
            },
        );
        assert_eq!(&result, "de_f");
        result = get_completion_token(
            &text,
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
        test_init();
        let server = LSPServer::new(None);
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
                version: 3,
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

        let completion_params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 4,
                    character: 3,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::INVOKED,
                trigger_character: None,
            }),
        };
        let response: CompletionResponse = server.completion(completion_params).unwrap();
        let item1 = CompletionItem {
            label: "abc".to_owned(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("logic".to_string()),
            ..CompletionItem::default()
        };
        let item2 = CompletionItem {
            label: "abcd".to_owned(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("logic".to_string()),
            ..CompletionItem::default()
        };
        if let CompletionResponse::List(item) = response {
            assert!(item.items.contains(&item1));
            assert!(item.items.contains(&item2));
        } else {
            panic!();
        }
    }

    #[test]
    fn test_nested_completion() {
        test_init();
        let server = LSPServer::new(None);
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

        let change_params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: 3,
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

        let completion_params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 4,
                    character: 3,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::INVOKED,
                trigger_character: None,
            }),
        };
        let response: CompletionResponse = server.completion(completion_params).unwrap();
        let item1 = CompletionItem {
            label: "abc".to_owned(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("logic".to_string()),
            ..CompletionItem::default()
        };
        let item3 = CompletionItem {
            label: "aouter".to_owned(),
            kind: Some(CompletionItemKind::VARIABLE),
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
            panic!();
        }
    }

    #[test]
    fn test_dot_completion() {
        test_init();
        let server = LSPServer::new(None);
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
                trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some(".".to_string()),
            }),
        };
        let response: CompletionResponse = server.completion(completion_params).unwrap();
        dbg!(&response);
        let item1 = CompletionItem {
            label: "abcd".to_owned(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("wire".to_string()),
            ..CompletionItem::default()
        };
        if let CompletionResponse::List(item) = response {
            eprintln!("{:#?}", item);
            assert!(item.items.contains(&item1));
            assert!(item.items.len() == 1);
        } else {
            panic!();
        }
        let completion_params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 7,
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some(".".to_string()),
            }),
        };
        let response: CompletionResponse = server.completion(completion_params).unwrap();
        if let CompletionResponse::List(item) = response {
            eprintln!("{:#?}", item);
            assert!(item.items.contains(&item1));
            assert!(item.items.len() == 1);
        } else {
            panic!();
        }
    }

    #[test]
    fn test_trigger_dot_nocontext() {
        test_init();
        let server = LSPServer::new(None);
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
            context: None,
        };
        let response: CompletionResponse = server.completion(completion_params).unwrap();
        dbg!(&response);
        let item1 = CompletionItem {
            label: "abcd".to_owned(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("wire".to_string()),
            ..CompletionItem::default()
        };
        if let CompletionResponse::List(item) = response {
            eprintln!("{:#?}", item);
            assert!(item.items.contains(&item1));
            assert!(item.items.len() == 1);
        } else {
            panic!();
        }
        let completion_params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 7,
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some(".".to_string()),
            }),
        };
        let response: CompletionResponse = server.completion(completion_params).unwrap();
        if let CompletionResponse::List(item) = response {
            eprintln!("{:#?}", item);
            assert!(item.items.contains(&item1));
            assert!(item.items.len() == 1);
        } else {
            panic!();
        }
    }

    #[test]
    fn test_dot_completion_instantiation() {
        test_init();
        let server = LSPServer::new(None);
        let text = r#"interface test_inter;
    wire wrong;
    logic clk;
endinterface
module test;
    logic clk;
    test_inter2 t (
        .clk(clk),
        .
    )
endmodule
module test1;
endmodule
interface test_inter2;
    wire abcd;
    logic clk;
endinterface
"#;

        let url = Url::parse("file:///test.sv").unwrap();
        server.did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: url.clone(),
                language_id: "systemverilog".to_owned(),
                version: 0,
                text: text.to_owned(),
            },
        });

        let completion_params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: url },
                position: Position {
                    line: 8,
                    character: 9,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some(".".to_string()),
            }),
        };
        let response: CompletionResponse = server.completion(completion_params).unwrap();
        dbg!(&response);

        if let CompletionResponse::List(list) = response {
            assert_eq!(
                list.items
                    .iter()
                    .map(|i| i.label.clone())
                    .collect::<Vec<String>>(),
                vec!["abcd", "clk"]
            );
        } else {
            panic!();
        }
    }

    #[test]
    fn test_inter_file_completion() {
        test_init();
        let server = LSPServer::new(None);
        let uri = Url::parse("file:///test.sv").unwrap();
        let uri2 = Url::parse("file:///test2.sv").unwrap();
        let text = r#"module test;
    s
endmodule
"#;
        let text2 = r#"interface simple_bus;
    logic clk;
endinterface"#;
        let open_params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "systemverilog".to_owned(),
                version: 0,
                text: text.to_owned(),
            },
        };
        let open_params2 = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri2.clone(),
                language_id: "systemverilog".to_owned(),
                version: 0,
                text: text2.to_owned(),
            },
        };
        server.did_open(open_params);
        server.did_open(open_params2);

        let completion_params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 1,
                    character: 5,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::INVOKED,
                trigger_character: None,
            }),
        };
        let response: CompletionResponse = server.completion(completion_params).unwrap();
        dbg!(&response);
        if let CompletionResponse::List(item) = response {
            // eprintln!("{:#?}", item);
            let names: Vec<&String> = item.items.iter().map(|x| &x.label).collect();
            assert!(names.contains(&&"simple_bus".to_string()));
        } else {
            panic!();
        }
    }
}
