use std::time::{Duration, Instant};
use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Query, QueryCursor, Tree};

#[derive(Debug, Copy, Clone)]
pub struct Symbol {
    ident_node: ByteRange,
    type_node: Option<ByteRange>,
    scope_node: ByteRange,
    parent: Option<ByteRange>,
    file: usize,
    ckind: Option<CompletionItemKind>,
    skind: Option<SymbolKind>,
    signed: bool,
    direction: PortDirection,
}

impl Symbol {
    fn is_port(&self) -> bool {
        self.direction != PortDirection::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ByteRange {
    start: usize,
    end: usize,
}

impl ByteRange {
    fn contains(&self, idx: usize) -> bool {
        return idx >= self.start && idx < self.end;
    }
}

impl From<std::ops::Range<usize>> for ByteRange {
    fn from(range: std::ops::Range<usize>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PortDirection {
    None,
    Input,
    Output,
    InOut,
    Ref,
    Interface((ByteRange, Option<ByteRange>)),
}

impl From<&str> for PortDirection {
    fn from(direction: &str) -> Self {
        match direction {
            "input" => PortDirection::Input,
            "output" => PortDirection::Output,
            "inout" => PortDirection::InOut,
            "ref" => PortDirection::Ref,
            _ => PortDirection::None,
        }
    }
}

struct SymbolBuilder {
    ident_node: Option<ByteRange>,
    type_node: Option<ByteRange>,
    scope_node: Option<ByteRange>,
    parent: Option<ByteRange>,
    file: usize,
    ckind: Option<CompletionItemKind>,
    skind: Option<SymbolKind>,
    signed: bool,
    direction: PortDirection,
}

impl SymbolBuilder {
    fn new(file: usize) -> Self {
        Self {
            ident_node: None,
            type_node: None,
            scope_node: None,
            parent: None,
            file,
            ckind: None,
            skind: None,
            signed: false,
            direction: PortDirection::None,
        }
    }

    /// Set the symbol builder's ident node.
    fn ident_node(&mut self, ident_node: ByteRange) {
        self.ident_node = Some(ident_node);
    }

    /// Set the symbol builder's type node.
    fn type_node(&mut self, type_node: ByteRange) {
        self.type_node = Some(type_node);
    }

    /// Set the symbol builder's scope node.
    fn scope_node(&mut self, scope_node: ByteRange) {
        self.scope_node = Some(scope_node);
    }

    /// Set the symbol builder's parent.
    fn parent(&mut self, parent: ByteRange) {
        self.parent = Some(parent);
    }

    /// Set the symbol builder's ckind.
    fn kind(&mut self, kind: Option<(CompletionItemKind, SymbolKind)>) {
        self.ckind = kind.map(|k| k.0);
        self.skind = kind.map(|k| k.1);
    }

    fn build(self) -> Option<Symbol> {
        Some(Symbol {
            ident_node: self.ident_node?,
            type_node: self.type_node,
            scope_node: self.scope_node?,
            parent: self.parent,
            file: self.file,
            ckind: self.ckind,
            skind: self.skind,
            signed: self.signed,
            direction: self.direction,
        })
    }

    /// Set the symbol builder's signed.
    fn signed(&mut self) {
        self.signed = true;
    }

    /// Set the symbol builder's direction.
    fn direction(&mut self, direction: &str) {
        self.direction = direction.into();
    }

    fn direction_interface(&mut self, interface: ByteRange) {
        self.direction = PortDirection::None;
        self.direction = PortDirection::Interface((interface, None));
    }

    fn direction_modport(&mut self, modport: ByteRange) {
        match &self.direction {
            PortDirection::Interface((i, _)) => {
                self.direction = PortDirection::Interface((i.clone(), Some(modport)))
            }
            _ => (),
        }
    }
}

pub fn parse(text: &str) -> Option<Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(tree_sitter_verilog::language()).ok()?;
    parser.parse(text, None)
}

pub fn index(text: &str, tree: &Tree, query: &Query) -> Vec<Symbol> {
    let now = Instant::now();
    let mut symbols: Vec<Symbol> = Vec::new();
    let text_callback = move |node: Node| &text.as_bytes()[node.start_byte()..node.end_byte()];
    let mut cursor = QueryCursor::new();
    let mats = cursor.matches(&query, tree.root_node(), text_callback);
    let mut parent: Option<(ByteRange, ByteRange)> = None;
    for m in mats {
        // scopes
        if m.pattern_index == 0 {
            let mut builder = SymbolBuilder::new(0);
            for cap in m.captures {
                match query.capture_names()[cap.index as usize].as_str() {
                    "ident" => builder.ident_node(cap.node.byte_range().into()),
                    "keyword" => {
                        builder.type_node(cap.node.byte_range().into());
                        builder.kind(match cap.node.utf8_text(text.as_bytes()).unwrap_or("") {
                            "module" | "macromodule" => {
                                Some((CompletionItemKind::Module, SymbolKind::Module))
                            }
                            "interface" => {
                                Some((CompletionItemKind::Interface, SymbolKind::Interface))
                            }
                            "task" => Some((CompletionItemKind::Function, SymbolKind::Function)),
                            "function" => {
                                Some((CompletionItemKind::Function, SymbolKind::Function))
                            }
                            "package" => Some((CompletionItemKind::Module, SymbolKind::Package)),
                            "class" => Some((CompletionItemKind::Class, SymbolKind::Class)),
                            "struct" => Some((CompletionItemKind::Struct, SymbolKind::Struct)),
                            "union" => Some((CompletionItemKind::Struct, SymbolKind::Struct)),
                            "enum" => Some((CompletionItemKind::Enum, SymbolKind::Enum)),
                            _ => None,
                        });
                    }
                    "scope" => builder.scope_node(cap.node.byte_range().into()),
                    _ => (),
                }
            }
            let symbol = builder.build();
            symbol.as_ref().map(|s| {
                parent = Some((s.ident_node, s.scope_node));
            });
            symbols.extend(symbol);
        }
        // ports
        else if m.pattern_index == 1 {
            let mut builder = SymbolBuilder::new(0);
            builder.direction("inout");
            for cap in m.captures {
                match query.capture_names()[cap.index as usize].as_str() {
                    "ident" => {
                        builder.ident_node(cap.node.byte_range().into());
                        if let Some((pi, pr)) = parent {
                            if pr.contains(cap.node.start_byte().into()) {
                                builder.parent(pi);
                            }
                        }
                    }
                    "port" => builder.scope_node(cap.node.byte_range().into()),
                    "type" => builder.type_node(cap.node.byte_range().into()),
                    "direction" => {
                        builder.direction(cap.node.utf8_text(text.as_bytes()).unwrap_or(""))
                    }
                    "signed" => builder.signed(),
                    "interface" => builder.direction_interface(cap.node.byte_range().into()),
                    "modport" => builder.direction_modport(cap.node.byte_range().into()),
                    _ => (),
                }
            }
            builder.kind(Some((CompletionItemKind::Property, SymbolKind::Property)));
            symbols.extend(builder.build());
        }
    }
    println!(
        "indexed {} symbols in {} ms",
        symbols.len(),
        now.elapsed().as_millis()
    );
    symbols
}

const SYMBOL_QUERY: &str = r#"
[
    (module_declaration
        (_
            (module_keyword) @keyword
            (simple_identifier) @ident))
    (program_declaration
        (_
            "program" @keyword
            (program_identifier) @ident))
    (interface_declaration
        (_
            "interface" @keyword
            (interface_identifier) @ident))
    (checker_declaration
        "checker" @keyword
        (checker_identifier) @ident)
    (covergroup_declaration
        "covergroup" @keyword
        (covergroup_identifier) @ident)
    (task_declaration
        "task" @keyword
        (_
            (task_identifier) @ident))
    (function_declaration
        "function" @keyword
        (_
            (function_identifier) @ident))
    (package_declaration
        "package" @keyword
        (package_identifier) @ident)
    (class_declaration
        "class" @keyword
        (class_identifier) @ident)
    (data_declaration
        (data_type_or_implicit1
            (data_type
                "enum" @keyword))
        (list_of_variable_decl_assignments
            (variable_decl_assignment
                (simple_identifier) @ident)))
    (data_declaration
        (data_type_or_implicit1
            (data_type
                (struct_union) @keyword))
        (list_of_variable_decl_assignments
            (variable_decl_assignment
                (simple_identifier) @ident)))
] @scope

[
(ansi_port_declaration
    [
    (variable_port_header
        (port_direction)? @direction
        [
        (data_type)? @type
        ("var"? @type (data_type_or_implicit1))
        ]?)
    (net_port_header1
        (port_direction)? @direction
        (net_port_type1
            [
            (data_type_or_implicit1
                [
                (data_type) @type
                (implicit_data_type1
                    "signed" @signed)
                ])
            (net_type) @type
            (simple_identifier) @type
            ])?)
    (interface_port_header
        (interface_identifier) @interface
        (modport_identifier)? @modport) @type
    (port_direction) @direction
    ]
    (port_identifier) @ident)
(port_declaration
    (_
        ["input" "output" "inout" "ref"] @direction
        [
        ("var"? @type (data_type_or_implicit1))
        (net_port_type1
            [
            (data_type_or_implicit1
                [
                (data_type) @type
                (implicit_data_type1
                    "signed" @signed)
                ])
            (net_type) @type
            (simple_identifier) @type
            ])
            (data_type_or_implicit1
                [
                (data_type) @type
                (implicit_data_type1
                    "signed" @signed)
                ])
        (data_type) @type
        ]?
        [
        (list_of_port_identifiers
            (port_identifier) @ident)
        (list_of_variable_identifiers
            (simple_identifier) @ident)
        ]))
(port_declaration
    (interface_port_declaration
        (interface_identifier) @interface
        (modport_identifier)? @modport
        (list_of_interface_identifiers
            (interface_identifier) @ident)))
] @port
"#;

fn range_text(range: ByteRange, text: &str) -> &str {
    let ret = std::str::from_utf8(&text.as_bytes()[range.start..range.end]);
    match ret {
        Ok(s) => s,
        Err(e) => {
            println!("{}", e);
            ""
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(
        query: &Query,
        port_str: &str,
        direction: &str,
        signed: bool,
        type_str: &str,
        ident_str: &str,
        parent_str: &str,
    ) {
        let text = format!(
            "
interface a;
    modport b (input c);
endinterface
module test ({});
endmodule
        ",
            port_str
        );
        let tree = parse(&text).unwrap();
        let mut ind = index(&text, &tree, query);
        ind.retain(|s| s.is_port());
        assert!(ind.len() == 1);
        let port = ind.get(0).unwrap();
        dbg!(&port);
        assert_eq!(port.direction, direction.into());
        assert_eq!(port.signed, signed);
        assert_eq!(range_text(port.ident_node, &text), ident_str);
        if let Some(type_node) = port.type_node {
            assert_eq!(range_text(type_node, &text), type_str);
        } else {
            assert_eq!("", type_str);
        }
        assert_eq!(range_text(port.parent.unwrap(), &text), parent_str);
    }

    #[test]
    fn ansi_ports() {
        let query = &Query::new(tree_sitter_verilog::language(), SYMBOL_QUERY).unwrap();
        port(query, "wire x", "inout", false, "wire", "x", "test");
        port(query, "integer x", "inout", false, "integer", "x", "test");
        port(
            query,
            "inout integer x",
            "inout",
            false,
            "integer",
            "x",
            "test",
        );
        port(query, "[5:0] x", "inout", false, "", "x", "test");
        port(query, "input x", "input", false, "", "x", "test");
        port(query, "input var x", "input", false, "var", "x", "test");
        port(
            query,
            "input var integer x",
            "input",
            false,
            "var",
            "x",
            "test",
        );
        port(query, "output x", "output", false, "", "x", "test");
        port(query, "output var x", "output", false, "var", "x", "test");
        port(
            query,
            "output integer x",
            "output",
            false,
            "integer",
            "x",
            "test",
        );
        port(query, "ref [5:0] x", "ref", false, "", "x", "test");
        port(query, "ref x [5:0]", "ref", false, "", "x", "test");
    }
}
