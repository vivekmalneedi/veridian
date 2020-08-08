use crate::server::LSPServer;
use crate::sources::LSPSupport;
use ropey::{Rope, RopeSlice};
use sv_parser::*;
use tower_lsp::lsp_types::*;

impl LSPServer {
    pub fn goto_definition(
        &mut self,
        params: GotoDefinitionParams,
    ) -> Option<GotoDefinitionResponse> {
        let doc = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let file_id = self.srcs.get_id(&doc).to_owned();
        self.srcs.update_parse_data(file_id);
        let scope = self.srcs.get_scope(file_id, &pos).unwrap();
        let file = self.srcs.get_file(file_id).unwrap();
        let token = get_definition_token(file.text.line(pos.line as usize), pos);
        for def in &scope.defs {
            if def.0 == token {
                let def_pos = file.text.byte_to_pos(def.1);
                return Some(GotoDefinitionResponse::Scalar(Location::new(
                    doc,
                    Range::new(def_pos, def_pos),
                )));
            }
        }
        None
    }

    pub fn hover(&mut self, params: HoverParams) -> Option<Hover> {
        let doc = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let file_id = self.srcs.get_id(&doc).to_owned();
        self.srcs.update_parse_data(file_id);
        let scope = self.srcs.get_scope(file_id, &pos).unwrap();
        let file = self.srcs.get_file(file_id).unwrap();
        let token = get_definition_token(file.text.line(pos.line as usize), pos);
        for def in &scope.defs {
            if def.0 == token {
                let def_line = file.text.byte_to_line(def.1);
                return Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::LanguageString(LanguageString {
                        language: "systemverilog".to_owned(),
                        value: get_hover(&file.text, def_line),
                    })),
                    range: None,
                });
            }
        }
        None
    }
}

fn get_definition_token(line: RopeSlice, pos: Position) -> String {
    let mut token = String::new();
    let mut line_iter = line.chars();
    for _ in 0..(line.utf16_cu_to_char(pos.character as usize)) {
        line_iter.next();
    }
    let mut c = line_iter.prev();
    while !c.is_none() && c.unwrap().is_alphanumeric() {
        token.push(c.unwrap());
        c = line_iter.prev();
    }
    token = token.chars().rev().collect();
    line_iter = line.chars();
    for _ in 0..(line.utf16_cu_to_char(pos.character as usize)) {
        line_iter.next();
    }
    let mut c = line_iter.next();
    while !c.is_none() && c.unwrap().is_alphanumeric() {
        token.push(c.unwrap());
        c = line_iter.next();
    }
    token
}

fn get_identifer_data(syntax_tree: &SyntaxTree, ident: &Identifier) -> (String, usize) {
    let id = match ident {
        Identifier::SimpleIdentifier(x) => x.nodes.0,
        Identifier::EscapedIdentifier(x) => x.nodes.0,
    };
    let id_str = syntax_tree.get_str(&id).unwrap();
    let idb = syntax_tree.get_origin(&id).unwrap().1;
    (id_str.to_owned(), idb)
}

pub fn get_definitions(
    syntax_tree: &SyntaxTree,
    scope_idents: &Vec<(String, usize, usize)>,
) -> Vec<(String, usize)> {
    let mut definitions: Vec<(String, usize)> = Vec::new();
    'outer: for node in syntax_tree {
        match node {
            RefNode::VariableIdentifier(x) => {
                definitions.push(get_identifer_data(syntax_tree, &x.nodes.0));
            }
            RefNode::NetIdentifier(x) => {
                let defn = get_identifer_data(syntax_tree, &x.nodes.0);
                let mut scope_idents_def: Vec<(String, usize, usize)> = scope_idents
                    .iter()
                    .filter(|x| defn.1 >= x.1 && defn.1 <= x.2)
                    .map(|x| x.clone())
                    .collect();
                scope_idents_def.sort_by(|a, b| (a.2 - a.1).cmp(&(b.2 - b.1)));
                let scope_ident = scope_idents_def.get(0).unwrap();
                for def in &definitions {
                    if (scope_ident.1 <= def.1) && (def.1 <= scope_ident.2) && def.0 == defn.0 {
                        continue 'outer;
                    }
                }
                definitions.push(defn);
            }
            RefNode::PortIdentifier(x) => {
                let defn = get_identifer_data(syntax_tree, &x.nodes.0);
                let mut scope_idents_def: Vec<(String, usize, usize)> = scope_idents
                    .iter()
                    .filter(|x| defn.1 >= x.1 && defn.1 <= x.2)
                    .map(|x| x.clone())
                    .collect();
                scope_idents_def.sort_by(|a, b| (a.2 - a.1).cmp(&(b.2 - b.1)));
                let scope_ident = scope_idents_def.get(0).unwrap();

                for def in &definitions {
                    if (scope_ident.1 <= def.1) && (def.1 <= scope_ident.2) && def.0 == defn.0 {
                        continue 'outer;
                    }
                }
                definitions.push(defn);
            }
            _ => (),
        }
    }
    definitions
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
        let line = Rope::from_str("assign abc[2:0] = 3'b000;");
        let token = get_definition_token(line.line(0), Position::new(0, 8));
        assert_eq!(token, "abc".to_owned());
    }

    #[test]
    fn test_get_definition() {
        let text = r#"module test;
  logic abc;
  assign abc = 1'b1;
  endmodule"#;
        // let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // d.push("tests_rtl/FIR_filter.sv");
        // let text = read_to_string(d).unwrap();
        let doc = Rope::from_str(&text);
        let syntax_tree = parse(doc.clone(), &Url::parse("file:///test.sv").unwrap()).unwrap();
        let scope_idents = get_scope_idents(&syntax_tree);
        let defs = get_definitions(&syntax_tree, &scope_idents);
        for def in defs {
            if def.0 == "abc".to_owned() {
                assert_eq!(doc.byte_to_pos(def.1), Position::new(1, 8));
            }
        }
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
