use crate::server::LSPServer;
use crate::sources::LSPSupport;
use ropey::{Rope, RopeSlice};
use std::fmt::Display;
use std::sync::Arc;
use sv_parser::*;
use tower_lsp::lsp_types::*;

mod def_types;
pub use def_types::*;

mod parse_defs;
use parse_defs::*;

impl LSPServer {
    pub fn goto_definition(&self, params: GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
        let doc = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let file_id = self.srcs.get_id(&doc).to_owned();
        let file = self.srcs.get_file(file_id)?;
        let file = file.read().ok()?;
        let token = get_definition_token(file.text.line(pos.line as usize), pos);
        let def = file
            .scope_tree
            .as_ref()?
            .get_definition(&token, file.text.pos_to_byte(&pos))?;
        let def_pos = file.text.byte_to_pos(def.byte_idx());
        Some(GotoDefinitionResponse::Scalar(Location::new(
            doc,
            Range::new(def_pos, def_pos),
        )))
    }

    pub fn hover(&self, params: HoverParams) -> Option<Hover> {
        let doc = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let file_id = self.srcs.get_id(&doc).to_owned();
        let file = self.srcs.get_file(file_id)?;
        let file = file.read().ok()?;
        let token = get_definition_token(file.text.line(pos.line as usize), pos);
        let def = file
            .scope_tree
            .as_ref()?
            .get_definition(&token, file.text.pos_to_byte(&pos))?;
        let def_line = file.text.byte_to_line(def.byte_idx());
        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::LanguageString(LanguageString {
                language: "systemverilog".to_owned(),
                value: get_hover(&file.text, def_line),
            })),
            range: None,
        })
    }
}

fn get_definition_token(line: RopeSlice, pos: Position) -> String {
    let mut token = String::new();
    let mut line_iter = line.chars();
    for _ in 0..(line.utf16_cu_to_char(pos.character as usize)) {
        line_iter.next();
    }
    let mut c = line_iter.prev();
    while !c.is_none() && (c.unwrap().is_alphanumeric() || c.unwrap() == '_') {
        token.push(c.unwrap());
        c = line_iter.prev();
    }
    token = token.chars().rev().collect();
    line_iter = line.chars();
    for _ in 0..(line.utf16_cu_to_char(pos.character as usize)) {
        line_iter.next();
    }
    let mut c = line_iter.next();
    while !c.is_none() && (c.unwrap().is_alphanumeric() || c.unwrap() == '_') {
        token.push(c.unwrap());
        c = line_iter.next();
    }
    token
}

pub fn get_definitions(
    syntax_tree: &SyntaxTree,
    scope_idents: &Vec<(String, usize, usize)>,
) -> Option<Vec<Arc<dyn Definition>>> {
    eprintln!("{}", syntax_tree);

    let mut definitions: Vec<Arc<dyn Definition>> = Vec::new();
    let mut event_iter = syntax_tree.into_iter().event();
    while let Some(event) = event_iter.next() {
        match event {
            NodeEvent::Enter(node) => match node {
                RefNode::AnsiPortDeclaration(n) => {
                    let port = port_dec_ansi(syntax_tree, n, &mut event_iter);
                    if port.is_some() {
                        definitions.push(Arc::new(port?));
                    }
                }
                RefNode::PortDeclaration(n) => {
                    let ports = port_dec_non_ansi(syntax_tree, n, &mut event_iter);
                    if ports.is_some() {
                        for port in ports? {
                            definitions.push(Arc::new(port));
                        }
                    }
                }
                RefNode::NetDeclaration(n) => {
                    let nets = net_dec(syntax_tree, n, &mut event_iter);
                    if nets.is_some() {
                        for net in nets? {
                            definitions.push(Arc::new(net));
                        }
                    }
                }
                RefNode::DataDeclaration(n) => {
                    let vars = data_dec(syntax_tree, n, &mut event_iter);
                    if vars.is_some() {
                        for var in vars? {
                            definitions.push(Arc::new(var));
                        }
                    }
                }
                RefNode::FunctionDeclaration(n) => {
                    let decs = function_dec(syntax_tree, n, &mut event_iter);
                    if decs.is_some() {
                        definitions.append(&mut decs?);
                    }
                }
                RefNode::TaskDeclaration(n) => {
                    let decs = task_dec(syntax_tree, n, &mut event_iter);
                    if decs.is_some() {
                        definitions.append(&mut decs?);
                    }
                }
                RefNode::ModportDeclaration(n) => {
                    let decs = modport_dec(syntax_tree, n, &mut event_iter);
                    if decs.is_some() {
                        for dec in decs? {
                            definitions.push(Arc::new(dec));
                        }
                    }
                }
                RefNode::ModuleInstantiation(n) => {
                    let decs = module_inst(syntax_tree, n, &mut event_iter);
                    if decs.is_some() {
                        for dec in decs? {
                            definitions.push(Arc::new(dec));
                        }
                    }
                }
                _ => (),
            },
            NodeEvent::Leave(_) => (),
        }
    }
    Some(definitions)
}

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
            } else if currentl.starts_with("//") {
                valid = true;
            } else if multiline {
                valid = true;
            } else {
                valid = false;
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
    use crate::completion::get_scope_idents;
    use crate::sources::{parse, LSPSupport};
    use ropey::Rope;
    use std::collections::HashMap;
    use std::fs::read_to_string;
    use std::path::PathBuf;

    #[test]
    fn test_definition_token() {
        let line = Rope::from_str("assign ab_c[2:0] = 3'b000;");
        let token = get_definition_token(line.line(0), Position::new(0, 10));
        assert_eq!(token, "ab_c".to_owned());
    }

    #[test]
    fn test_get_definition() {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("tests_rtl/definition_test.sv");
        let text = read_to_string(d).unwrap();
        let doc = Rope::from_str(&text);
        let syntax_tree = parse(
            &doc.clone(),
            &Url::parse("file:///tests_rtl/definition_test.sv").unwrap(),
            &None,
        )
        .unwrap();
        let scope_idents = get_scope_idents(&syntax_tree);
        let defs = get_definitions(&syntax_tree, &scope_idents).unwrap();
        for def in &defs {
            println!("{:?} {:?}", def, doc.byte_to_pos(def.byte_idx()));
        }
        /*
        let token = get_definition_token(doc.line(3), Position::new(3, 13));
        for def in defs {
            if token == def.ident {
                assert_eq!(doc.byte_to_pos(def.byte_idx), Position::new(3, 9))
            }
        }
        */
        assert!(false);
    }

    #[test]
    fn test_hover() {
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
}
