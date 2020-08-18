use crate::server::LSPServer;
use crate::sources::LSPSupport;
use ropey::{Rope, RopeSlice};
use std::fmt::Display;
use sv_parser::*;
use tower_lsp::lsp_types::*;
use CaptureMode::{Post, Pre};

impl LSPServer {
    pub fn goto_definition(&self, params: GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
        let doc = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let file_id = self.srcs.get_id(&doc).to_owned();
        let file = self.srcs.get_file(file_id)?;
        let file = file.read().ok()?;
        eprintln!("def: read locked file");
        let scope = file.get_scope(&pos)?;
        let token = get_definition_token(file.text.line(pos.line as usize), pos);
        for def in &scope.defs {
            if def.ident == token {
                let def_pos = file.text.byte_to_pos(def.byte_idx);
                return Some(GotoDefinitionResponse::Scalar(Location::new(
                    doc,
                    Range::new(def_pos, def_pos),
                )));
            }
        }
        None
    }

    pub fn hover(&self, params: HoverParams) -> Option<Hover> {
        let doc = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let file_id = self.srcs.get_id(&doc).to_owned();
        let file = self.srcs.get_file(file_id)?;
        let file = file.read().ok()?;
        eprintln!("hover: read locked file");
        let scope = file.get_scope(&pos)?;
        let token = get_definition_token(file.text.line(pos.line as usize), pos);
        for def in &scope.defs {
            if def.ident == token {
                let def_line = file.text.byte_to_line(def.byte_idx);
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

#[derive(Eq, PartialEq, Debug)]
enum CaptureMode {
    Pre,
    Post,
    Full,
    No,
}

#[derive(Debug, Clone)]
pub struct Definition {
    pub ident: String,
    pub byte_idx: usize,
    pub type_str: String,
    pub kind: CompletionItemKind,
}

fn clean_type_str(type_str: String, ident: &str) -> String {
    let endings: &[_] = &[';', ','];
    let eq_offset = type_str.find('=').unwrap_or(type_str.len());
    let mut result = type_str.clone();
    result.replace_range(eq_offset.., "");
    result
        .trim_end()
        .trim_end_matches(endings)
        .trim_end_matches(ident)
        .to_string()
}

macro_rules! dec_list {
    ($loop:tt, $cap_args:ident, $event:ident, $start:path, ($($end:path),*), ($($mid:path),*), $kind:expr) => {
        match &$event {
            NodeEvent::Enter(node) => match node {
                $start(_) => {
                    $cap_args.capture = CaptureMode::Pre;
                    $cap_args.kind = $kind;
                    // eprintln!("startlst");
                }
                $($mid(ident_node) => match $cap_args.capture {
                    CaptureMode::Pre | CaptureMode::Post => {
                            if !$cap_args.post_dec.is_empty() {
                                let result = Definition{
                                    ident: $cap_args.ident.clone(),
                                    byte_idx: $cap_args.byte_idx,
                                    type_str: clean_type_str(format!(
                                        "{} {}{}",
                                        $cap_args.pre_dec.trim_end(),
                                        $cap_args.ident,
                                        $cap_args.post_dec,
                                    ), &$cap_args.ident),
                                    kind: $kind,
                                };
                                // eprintln!("midlst: {:?}", result);
                                $cap_args.defs.push(result);
                                $cap_args.post_dec.clear();
                            }
                            $cap_args.capture = CaptureMode::Post;
                            $cap_args.skip = true;
                            let loc = unwrap_locate!(*ident_node).unwrap();
                            $cap_args.ident =
                                $cap_args.tree.get_str(loc).unwrap().trim_end().to_string();
                            $cap_args.byte_idx = $cap_args.tree.get_origin(loc).unwrap().1;
                            continue $loop;
                    }
                    _ => (),
                },)*
                _ => (),
            },
            NodeEvent::Leave(node) => match node {
                $($end(_) => {
                    if !$cap_args.pre_dec.trim_start().is_empty(){
                        let result = Definition{
                            ident: $cap_args.ident.clone(),
                            byte_idx: $cap_args.byte_idx,
                            type_str: clean_type_str(format!(
                                "{} {}{}",
                                $cap_args.pre_dec.trim_end(),
                                $cap_args.ident,
                                $cap_args.post_dec), &$cap_args.ident),
                            kind: $kind,
                        };
                        // eprintln!("endlst: {:?}", result);
                        $cap_args.defs.push(result);
                    }
                    $cap_args.post_dec.clear();
                    $cap_args.pre_dec.clear();
                    $cap_args.ident.clear();
                    $cap_args.capture = CaptureMode::No;
                },)*
                _ => (),
            },
        };
    };
}

macro_rules! dec_full {
    ($loop:tt, $cap_args:ident, $event:ident, $start:path, ($($mid:path),*), $kind:expr) => {
        match &$event {
            NodeEvent::Enter(node) => match node {
                $start(_) => {
                    $cap_args.capture = CaptureMode::Full;
                    $cap_args.kind = $kind;
                    // eprintln!("startfull");
                }
                $($mid(ident_node) => match $cap_args.capture {
                    CaptureMode::Full => {
                            let loc = unwrap_locate!(*ident_node).unwrap();
                            $cap_args.ident =
                                $cap_args.tree.get_str(loc).unwrap().trim_end().to_string();
                            // eprintln!("ident:{}", $cap_args.ident);
                            $cap_args.byte_idx = $cap_args.tree.get_origin(loc).unwrap().1;
                            continue $loop;
                    }
                    _ => (),
                },)*
                _ => (),
            },
            _ => (),
        };
    };
}

pub fn get_definitions(
    syntax_tree: &SyntaxTree,
    scope_idents: &Vec<(String, usize, usize)>,
) -> Vec<Definition> {
    // eprintln!("{}", syntax_tree);

    struct CaptureArgs<'a> {
        tree: &'a SyntaxTree,
        pre_dec: String,
        post_dec: String,
        capture: CaptureMode,
        skip: bool,
        byte_idx: usize,
        ident: String,
        kind: CompletionItemKind,
        defs: Vec<Definition>,
    }
    impl<'a> CaptureArgs<'a> {
        fn new(tree: &SyntaxTree) -> CaptureArgs {
            CaptureArgs {
                tree,
                pre_dec: String::new(),
                post_dec: String::new(),
                capture: CaptureMode::No,
                skip: false,
                byte_idx: 0,
                ident: "".into(),
                kind: CompletionItemKind::Field,
                defs: Vec::new(),
            }
        }
    }

    let mut capture_args = CaptureArgs::new(syntax_tree);

    'outer: for event in syntax_tree.into_iter().event() {
        //TODO: handle interface port
        dec_list!(
            'outer,
            capture_args,
            event,
            RefNode::PortDeclaration,
            (RefNode::PortDeclaration),
            (RefNode::PortIdentifier, RefNode::VariableIdentifier),
             CompletionItemKind::Property
        );
        dec_list!(
            'outer,
            capture_args,
            event,
            RefNode::DataDeclarationVariable,
            (RefNode::DataDeclarationVariable),
            (RefNode::VariableIdentifier),
            CompletionItemKind::Variable
        );
        dec_list!(
            'outer,
            capture_args,
            event,
            RefNode::NetDeclaration,
            (RefNode::NetDeclaration),
            (RefNode::NetIdentifier),
            CompletionItemKind::Variable
        );
        dec_full!(
            'outer,
            capture_args,
            event,
            RefNode::FunctionDeclaration,
            (RefNode::FunctionIdentifier),
            CompletionItemKind::Function
        );
        dec_full!(
            'outer,
            capture_args,
            event,
            RefNode::TaskDeclaration,
            (RefNode::TaskIdentifier),
            CompletionItemKind::Function
        );
        match event {
            NodeEvent::Enter(node) => match node {
                RefNode::Locate(loc) => {
                    // eprintln!("capture: {:?}", capture_args.capture);
                    if !capture_args.skip {
                        let token = capture_args.tree.get_str(loc).unwrap().trim_end();
                        match capture_args.capture {
                            CaptureMode::Pre => {
                                if token.chars().count() > 1 {
                                    if capture_args.pre_dec.len() > 0
                                        && capture_args.pre_dec.chars().last().unwrap() != ' '
                                    {
                                        capture_args.pre_dec.push(' ');
                                    }
                                    capture_args.pre_dec.push_str(token);
                                    capture_args.pre_dec.push(' ');
                                } else {
                                    capture_args.pre_dec.push_str(token);
                                }
                            }
                            CaptureMode::Post => {
                                if token.chars().count() > 1
                                    && capture_args.post_dec.len() > 0
                                    && capture_args.post_dec.chars().last().unwrap() != ' '
                                {
                                    capture_args.post_dec.push(' ');
                                }
                                capture_args.post_dec.push_str(token);
                            }
                            CaptureMode::Full => {
                                if token == ";" {
                                    let full_def = Definition {
                                        ident: capture_args.ident.clone(),
                                        byte_idx: capture_args.byte_idx,
                                        type_str: clean_type_str(
                                            capture_args.pre_dec.clone(),
                                            &capture_args.ident,
                                        ),
                                        kind: capture_args.kind,
                                    };
                                    // eprintln!("endfull: {:?}", full_def);
                                    capture_args.defs.push(full_def);
                                    capture_args.capture = CaptureMode::No;
                                    capture_args.ident.clear();
                                    capture_args.pre_dec.clear();
                                    capture_args.post_dec.clear();
                                } else {
                                    if token.chars().count() > 1 {
                                        if capture_args.pre_dec.len() > 0
                                            && capture_args.pre_dec.chars().last().unwrap() != ' '
                                        {
                                            capture_args.pre_dec.push(' ');
                                        }
                                        capture_args.pre_dec.push_str(token);
                                        capture_args.pre_dec.push(' ');
                                    } else {
                                        capture_args.pre_dec.push_str(token);
                                    }
                                }
                            }
                            CaptureMode::No => (),
                        }
                    }
                    capture_args.skip = false;
                }
                _ => (),
            },
            _ => (),
        }
    }
    capture_args.defs
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
            doc.clone(),
            &Url::parse("file:///tests_rtl/definition_test.sv").unwrap(),
            None,
        )
        .unwrap();
        let scope_idents = get_scope_idents(&syntax_tree);
        let defs = get_definitions(&syntax_tree, &scope_idents);
        for def in &defs {
            println!("{:?} {:?}", def, doc.byte_to_pos(def.byte_idx));
        }
        let token = get_definition_token(doc.line(3), Position::new(3, 13));
        for def in defs {
            if token == def.ident {
                assert_eq!(doc.byte_to_pos(def.byte_idx), Position::new(3, 9))
            }
        }
        // assert!(false);
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
