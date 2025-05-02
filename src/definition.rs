use crate::server::LSPServer;
use crate::sources::LSPSupport;
use log::debug;
use ropey::{Rope, RopeSlice};
use tower_lsp::lsp_types::*;

use crate::symbol::*;

impl LSPServer {
    pub fn goto_definition(&self, params: GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
        let doc = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let files = self.srcs.files.lock().unwrap();
        let file = files.get(&doc)?;
        let token = get_definition_token(file.text.line(pos.line as usize), pos);
        drop(files);

        Some(GotoDefinitionResponse::Array(
            self.srcs.get_definition(&token, pos, &doc),
        ))
    }

    pub fn hover(&self, params: HoverParams) -> Option<Hover> {
        let doc = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let files = self.srcs.files.lock().unwrap();
        let file = files.get(&doc)?;
        let text = file.text.clone();
        let token = get_definition_token(file.text.line(pos.line as usize), pos);
        drop(files);

        let defs = self.srcs.get_definition(&token, pos, &doc);
        let def = defs.first()?;
        let def_line = def.range.start.line;
        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::LanguageString(LanguageString {
                language: "systemverilog".to_owned(),
                value: get_hover(&text, def_line as usize),
            })),
            range: None,
        })
    }

    pub fn document_symbol(&self, params: DocumentSymbolParams) -> Option<DocumentSymbolResponse> {
        let uri = params.text_document.uri;
        let binding = self.srcs.index.lock().ok()?;
        let file = binding.get(&uri)?;

        let mut stack: Vec<(Symbol, Vec<DocumentSymbol>)> = Vec::new();
        let mut top_level: Vec<DocumentSymbol> = Vec::new();

        for sym in &file.syms {
            let parent = match sym.parent {
                Some(p) => file.text.byte_slice(p).to_string(),
                None => "".to_string(),
            };
            let type_str = match sym.type_node {
                Some(p) => file.text.byte_slice(p).to_string(),
                None => "".to_string(),
            };
            debug!(
                "sym: {}, parent: {}, type: {}",
                file.text.byte_slice(sym.ident_node),
                parent,
                type_str
            );
            let doc_sym = sym.to_document_symbol(&file.text);
            // Clean up the stack: pop until current symbol fits in the scope
            while let Some((parent_sym, _)) = stack.last() {
                if let Some(scope) = parent_sym.scope_node {
                    if scope.contains(sym.ident_node.start) {
                        break;
                    }
                }
                let (_, children) = stack.pop().unwrap();
                if let Some((_, parent_children)) = stack.last_mut() {
                    let mut pc = parent_children.pop().unwrap();
                    pc.children.get_or_insert(Vec::new()).extend(children);
                    parent_children.push(pc);
                } else {
                    // top level
                    top_level.extend(children);
                }
            }

            // Add current symbol to stack
            stack.push((*sym, vec![doc_sym]));
        }

        // Flush remaining stack
        while let Some((_, children)) = stack.pop() {
            if let Some((_, parent_children)) = stack.last_mut() {
                let mut pc = parent_children.pop().unwrap();
                pc.children.get_or_insert(Vec::new()).extend(children);
                parent_children.push(pc);
            } else {
                top_level.extend(children);
            }
        }

        Some(DocumentSymbolResponse::Nested(top_level))
    }

    pub fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Option<Vec<DocumentHighlight>> {
        let uri = params.text_document_position_params.text_document.uri;
        let binding = self.srcs.index.lock().ok()?;
        let file = binding.get(&uri)?;

        let pos = params.text_document_position_params.position;
        let token = get_definition_token(file.text.line(pos.line as usize), pos);

        // Find all symbols with the same identifier
        let root_node = file.tree.root_node();
        let cursor = root_node.walk();
        let mut highlights = Vec::new();
        let mut stack = vec![cursor.node()];

        while let Some(node) = stack.pop() {
            if node.kind() == "simple_identifier"
                && file.text.byte_slice(node.byte_range()) == token
            {
                highlights.push(file.text.byte_range_to_range(node.byte_range().into()));
            }

            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    stack.push(child);
                }
            }
        }
        Some(
            highlights
                .iter()
                .map(|r| DocumentHighlight {
                    range: *r,
                    kind: Some(DocumentHighlightKind::TEXT),
                })
                .collect(),
        )
    }
}

/// retrieve the token the user invoked goto definition or hover on
fn get_definition_token(line: RopeSlice, pos: Position) -> String {
    let mut token = String::new();
    let mut line_iter = line.chars();
    for _ in 0..(line.utf16_cu_to_char(pos.character as usize)) {
        line_iter.next();
    }
    let mut c = line_iter.prev();
    while c.is_some() && (c.unwrap().is_alphanumeric() || c.unwrap() == '_') {
        token.push(c.unwrap());
        c = line_iter.prev();
    }
    token = token.chars().rev().collect();
    line_iter = line.chars();
    for _ in 0..(line.utf16_cu_to_char(pos.character as usize)) {
        line_iter.next();
    }
    let mut c = line_iter.next();
    while c.is_some() && (c.unwrap().is_alphanumeric() || c.unwrap() == '_') {
        token.push(c.unwrap());
        c = line_iter.next();
    }
    token
}

/// get the hover information
fn get_hover(doc: &Rope, line: usize) -> String {
    if line == 0 {
        return doc.line(line).to_string();
    }
    let mut hover: Vec<String> = Vec::new();
    let mut multiline: bool = false;
    let mut valid: bool = true;
    let mut current: String = doc.line(line).to_string();
    let ltrim: String = " ".repeat(current.len() - current.trim_start().len());
    let mut line_idx = line;

    // iterate upwards from the definition, and grab the comments
    while valid {
        hover.push(current.clone());
        line_idx -= 1;
        valid = false;
        if line_idx > 0 {
            current = doc.line(line_idx).to_string();
            let currentl = current.clone().trim_start().to_owned();
            let currentr = current.clone().trim_end().to_owned();
            if currentl.starts_with("/*") && currentr.ends_with("*/") {
                valid = true;
            } else if currentr.ends_with("*/") {
                multiline = true;
                valid = true;
            } else if currentl.starts_with("/*") {
                multiline = false;
                valid = true;
            } else {
                valid = currentl.starts_with("//") || multiline;
            }
        }
    }
    hover.reverse();
    let mut result: Vec<String> = Vec::new();
    for i in hover {
        if let Some(stripped) = i.strip_prefix(&ltrim) {
            result.push(stripped.to_owned());
        } else {
            result.push(i);
        }
    }
    result.join("").trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::test_init;
    use ropey::Rope;
    use std::fs::read_to_string;
    use std::path::PathBuf;

    #[test]
    fn test_definition_token() {
        test_init();
        let line = Rope::from_str("assign ab_c[2:0] = 3'b000;");
        let token = get_definition_token(line.line(0), Position::new(0, 10));
        assert_eq!(token, "ab_c".to_owned());
    }

    #[test]
    fn test_get_definition() {
        test_init();
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("test_data/definition_test.sv");
        let text = read_to_string(d).unwrap();
        let doc = Rope::from_str(&text);
        let url = Url::parse("file:///test_data/definition_test.sv").unwrap();
        let server = LSPServer::new(None);

        let open_params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: url.clone(),
                language_id: "systemverilog".to_owned(),
                version: 0,
                text: text.to_owned(),
            },
        };
        server.did_open(open_params);
        let resp = server
            .goto_definition(GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: url },
                    position: Position::new(3, 13),
                },
                work_done_progress_params: WorkDoneProgressParams {
                    work_done_token: None,
                },
                partial_result_params: PartialResultParams {
                    partial_result_token: None,
                },
            })
            .unwrap();

        let token = get_definition_token(doc.line(3), Position::new(3, 13));
        if let GotoDefinitionResponse::Array(defs) = resp {
            for def in defs {
                if token == doc.byte_slice(doc.range_to_byte_range(def.range)) {
                    assert_eq!(def.range.start, Position::new(3, 9))
                }
            }
        } else {
            panic!();
        }
    }

    #[test]
    fn test_definition_instance_port() {
        test_init();
        let text = r#"
interface a (
    input logic b
);
endinterface
module test;
    logic c;
    a intf (
        .b(c)
    );
endmodule"#;
        let url = Url::parse("file:///test.sv").unwrap();
        let server = LSPServer::new(None);
        let open_params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: url.clone(),
                language_id: "systemverilog".to_owned(),
                version: 0,
                text: text.to_owned(),
            },
        };
        server.did_open(open_params);

        let resp = server
            .goto_definition(GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: url },
                    position: Position::new(8, 9),
                },
                work_done_progress_params: WorkDoneProgressParams {
                    work_done_token: None,
                },
                partial_result_params: PartialResultParams {
                    partial_result_token: None,
                },
            })
            .unwrap();

        if let GotoDefinitionResponse::Array(defs) = resp {
            assert!(defs.len() == 1);
            for def in defs {
                assert_eq!(
                    def.range.start,
                    Position {
                        line: 2,
                        character: 16
                    }
                )
            }
        } else {
            panic!();
        }
    }

    #[test]
    fn test_hover() {
        test_init();
        let text = r#"
// module test
// test module
module test;
  /* a */
  logic a;
  /**
    * b
  */
  logic b;
  endmodule"#;
        let doc = Rope::from_str(text);
        eprintln!("{}", get_hover(&doc, 2));
        assert_eq!(
            get_hover(&doc, 3),
            r#"// module test
// test module
module test;"#
                .to_owned()
        );
        assert_eq!(
            get_hover(&doc, 9),
            r#"/**
  * b
*/
logic b;"#
                .to_owned()
        );
    }

    #[test]
    fn test_symbols() {
        test_init();
        let text = r#"
module test;
  logic a;
  logic b;
endmodule"#;
        let url = Url::parse("file:///test.sv").unwrap();
        let server = LSPServer::new(None);
        let open_params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: url.clone(),
                language_id: "systemverilog".to_owned(),
                version: 0,
                text: text.to_owned(),
            },
        };
        server.did_open(open_params);

        let symbols = server
            .document_symbol(DocumentSymbolParams {
                text_document: TextDocumentIdentifier { uri: url },
                work_done_progress_params: WorkDoneProgressParams {
                    work_done_token: None,
                },
                partial_result_params: PartialResultParams {
                    partial_result_token: None,
                },
            })
            .unwrap();
        if let DocumentSymbolResponse::Nested(syms) = symbols {
            let symbol = syms.first().unwrap();
            assert_eq!(&symbol.name, "test");
            let names: Vec<String> = symbol
                .children
                .as_ref()
                .unwrap()
                .iter()
                .map(|x| x.name.clone())
                .collect();
            assert!(names.contains(&"a".to_string()));
            assert!(names.contains(&"b".to_string()));
        } else {
            panic!();
        }
    }

    #[test]
    fn test_highlight() {
        test_init();
        let text = r#"
module test;
  logic clk;
  assign clk = 1'b1;
endmodule"#;
        let url = Url::parse("file:///test.sv").unwrap();
        let server = LSPServer::new(None);
        let open_params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: url.clone(),
                language_id: "systemverilog".to_owned(),
                version: 0,
                text: text.to_owned(),
            },
        };
        server.did_open(open_params);

        let highlights = server
            .document_highlight(DocumentHighlightParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: url },
                    position: Position::new(2, 8),
                },
                work_done_progress_params: WorkDoneProgressParams {
                    work_done_token: None,
                },
                partial_result_params: PartialResultParams {
                    partial_result_token: None,
                },
            })
            .unwrap();
        let expected = vec![
            DocumentHighlight {
                range: Range {
                    start: Position {
                        line: 3,
                        character: 9,
                    },
                    end: Position {
                        line: 3,
                        character: 12,
                    },
                },
                kind: Some(DocumentHighlightKind::TEXT),
            },
            DocumentHighlight {
                range: Range {
                    start: Position {
                        line: 2,
                        character: 8,
                    },
                    end: Position {
                        line: 2,
                        character: 11,
                    },
                },
                kind: Some(DocumentHighlightKind::TEXT),
            },
        ];
        assert_eq!(highlights, expected)
    }
}
