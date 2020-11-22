use crate::server::LSPServer;
use crate::sources::LSPSupport;
use log::{debug, trace};
use ropey::{Rope, RopeSlice};
use sv_parser::*;
use tower_lsp::lsp_types::*;

pub mod def_types;
pub use def_types::*;

mod parse_defs;
use parse_defs::*;

impl LSPServer {
    pub fn goto_definition(&self, params: GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
        let doc = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let file_id = self.srcs.get_id(&doc).to_owned();
        self.srcs.wait_parse_ready(file_id, false);
        let file = self.srcs.get_file(file_id)?;
        let file = file.read().ok()?;
        let token = get_definition_token(file.text.line(pos.line as usize), pos);
        debug!("goto definition, token: {}", &token);
        let scope_tree = self.srcs.scope_tree.read().ok()?;
        trace!("{:#?}", scope_tree.as_ref()?);
        let def = scope_tree
            .as_ref()?
            .get_definition(&token, file.text.pos_to_byte(&pos), &doc)?;
        let def_pos = file.text.byte_to_pos(def.byte_idx());
        debug!("def: {:?}", def_pos);
        Some(GotoDefinitionResponse::Scalar(Location::new(
            def.url(),
            Range::new(def_pos, def_pos),
        )))
    }

    pub fn hover(&self, params: HoverParams) -> Option<Hover> {
        let doc = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let file_id = self.srcs.get_id(&doc).to_owned();
        self.srcs.wait_parse_ready(file_id, false);
        let file = self.srcs.get_file(file_id)?;
        let file = file.read().ok()?;
        let token = get_definition_token(file.text.line(pos.line as usize), pos);
        debug!("hover, token: {}", &token);
        let scope_tree = self.srcs.scope_tree.read().ok()?;
        let def = scope_tree
            .as_ref()?
            .get_definition(&token, file.text.pos_to_byte(&pos), &doc)?;
        let def_line = file.text.byte_to_line(def.byte_idx());
        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::LanguageString(LanguageString {
                language: "systemverilog".to_owned(),
                value: get_hover(&file.text, def_line),
            })),
            range: None,
        })
    }

    pub fn document_symbol(&self, params: DocumentSymbolParams) -> Option<DocumentSymbolResponse> {
        let uri = params.text_document.uri;
        let file_id = self.srcs.get_id(&uri).to_owned();
        self.srcs.wait_parse_ready(file_id, false);
        let file = self.srcs.get_file(file_id)?;
        let file = file.read().ok()?;
        let scope_tree = self.srcs.scope_tree.read().ok()?;
        Some(DocumentSymbolResponse::Nested(
            scope_tree.as_ref()?.document_symbols(&uri, &file.text),
        ))
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

type ScopesAndDefs = Option<(Vec<Box<dyn Scope>>, Vec<Box<dyn Definition>>)>;

/// Take a given syntax node from a sv-parser syntax tree and extract out the definition/scope at
/// that point.
pub fn match_definitions(
    syntax_tree: &SyntaxTree,
    event_iter: &mut EventIter,
    node: RefNode,
    url: &Url,
) -> ScopesAndDefs {
    let mut definitions: Vec<Box<dyn Definition>> = Vec::new();
    let mut scopes: Vec<Box<dyn Scope>> = Vec::new();
    match node {
        RefNode::ModuleDeclaration(n) => {
            let module = module_dec(syntax_tree, n, event_iter, url);
            if module.is_some() {
                scopes.push(Box::new(module?));
            }
        }
        RefNode::InterfaceDeclaration(n) => {
            let interface = interface_dec(syntax_tree, n, event_iter, url);
            if interface.is_some() {
                scopes.push(Box::new(interface?));
            }
        }
        RefNode::UdpDeclaration(n) => {
            let dec = udp_dec(syntax_tree, n, event_iter, url);
            if dec.is_some() {
                scopes.push(Box::new(dec?));
            }
        }
        RefNode::ProgramDeclaration(n) => {
            let dec = program_dec(syntax_tree, n, event_iter, url);
            if dec.is_some() {
                scopes.push(Box::new(dec?));
            }
        }
        RefNode::PackageDeclaration(n) => {
            let dec = package_dec(syntax_tree, n, event_iter, url);
            if dec.is_some() {
                scopes.push(Box::new(dec?));
            }
        }
        RefNode::ConfigDeclaration(n) => {
            let dec = config_dec(syntax_tree, n, event_iter, url);
            if dec.is_some() {
                scopes.push(Box::new(dec?));
            }
        }
        RefNode::ClassDeclaration(n) => {
            let dec = class_dec(syntax_tree, n, event_iter, url);
            if dec.is_some() {
                scopes.push(Box::new(dec?));
            }
        }
        RefNode::PortDeclaration(n) => {
            let ports = port_dec_non_ansi(syntax_tree, n, event_iter, url);
            if ports.is_some() {
                for port in ports? {
                    definitions.push(Box::new(port));
                }
            }
        }
        RefNode::NetDeclaration(n) => {
            let nets = net_dec(syntax_tree, n, event_iter, url);
            if nets.is_some() {
                for net in nets? {
                    definitions.push(Box::new(net));
                }
            }
        }
        RefNode::DataDeclaration(n) => {
            let vars = data_dec(syntax_tree, n, event_iter, url);
            if vars.is_some() {
                definitions.append(&mut vars?);
            }
        }
        RefNode::ParameterDeclaration(n) => {
            let vars = param_dec(syntax_tree, n, event_iter, url);
            if vars.is_some() {
                for var in vars? {
                    definitions.push(Box::new(var));
                }
            }
        }
        RefNode::LocalParameterDeclaration(n) => {
            let vars = localparam_dec(syntax_tree, n, event_iter, url);
            if vars.is_some() {
                for var in vars? {
                    definitions.push(Box::new(var));
                }
            }
        }
        RefNode::FunctionDeclaration(n) => {
            let dec = function_dec(syntax_tree, n, event_iter, url);
            if dec.is_some() {
                scopes.push(Box::new(dec?));
            }
        }
        RefNode::TaskDeclaration(n) => {
            let dec = task_dec(syntax_tree, n, event_iter, url);
            if dec.is_some() {
                scopes.push(Box::new(dec?));
            }
        }
        RefNode::ModportDeclaration(n) => {
            let decs = modport_dec(syntax_tree, n, event_iter, url);
            if decs.is_some() {
                for dec in decs? {
                    definitions.push(Box::new(dec));
                }
            }
        }
        RefNode::ModuleInstantiation(n) => {
            let decs = module_inst(syntax_tree, n, event_iter, url);
            if decs.is_some() {
                for dec in decs? {
                    definitions.push(Box::new(dec));
                }
            }
        }
        RefNode::TextMacroDefinition(n) => {
            let dec = text_macro_def(syntax_tree, n, event_iter, url);
            if dec.is_some() {
                definitions.push(Box::new(dec?));
            }
        }
        _ => (),
    }
    Some((scopes, definitions))
}

/// convert the syntax tree to a scope tree
/// the root node is the global scope
pub fn get_scopes(syntax_tree: &SyntaxTree, url: &Url) -> Option<GenericScope> {
    trace!("{}", syntax_tree);
    let mut scopes: Vec<Box<dyn Scope>> = Vec::new();
    let mut global_scope: GenericScope = GenericScope::new(url);
    global_scope.ident = "global".to_string();
    let mut event_iter = syntax_tree.into_iter().event();
    // iterate over each enter event and extract out any scopes or definitions
    // match_definitions is recursively called so we get a tree in the end
    while let Some(event) = event_iter.next() {
        match event {
            NodeEvent::Enter(node) => {
                let mut result = match_definitions(syntax_tree, &mut event_iter, node, url)?;
                global_scope.defs.append(&mut result.1);
                scopes.append(&mut result.0);
            }
            NodeEvent::Leave(_) => (),
        }
    }
    global_scope.scopes.append(&mut scopes);
    Some(global_scope)
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
            } else if currentl.starts_with("//") || multiline {
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
    use crate::sources::{parse, LSPSupport};
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
        let syntax_tree = parse(&doc, &url, &None, &Vec::new()).unwrap();
        trace!("{}", &syntax_tree);
        let scope_tree = get_scopes(&syntax_tree, &url).unwrap();
        trace!("{:#?}", &scope_tree);
        for def in &scope_tree.defs {
            trace!("{:?} {:?}", def, doc.byte_to_pos(def.byte_idx()));
        }
        let token = get_definition_token(doc.line(3), Position::new(3, 13));
        for def in scope_tree.defs {
            if token == def.ident() {
                assert_eq!(doc.byte_to_pos(def.byte_idx()), Position::new(3, 9))
            }
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
        let text = r#"
module test;
  logic a;
  logic b;
endmodule"#;
        let doc = Rope::from_str(&text);
        let url = Url::parse("file:///test.sv").unwrap();
        let syntax_tree = parse(&doc, &url, &None, &Vec::new()).unwrap();
        let scope_tree = get_scopes(&syntax_tree, &url).unwrap();
        let symbol = scope_tree.document_symbols(&url, &doc);
        let symbol = symbol.get(0).unwrap();
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
    }
}
