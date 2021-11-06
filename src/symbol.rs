use ropey::{iter::Bytes, Rope};
use std::time::{Duration, Instant};
use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Point, Query, QueryCursor, QueryError, TextProvider, Tree};

#[derive(Debug, Copy, Clone)]
pub struct Symbol {
    ident_node: ByteRange,
    type_node: Option<ByteRange>,
    scope_node: Option<ByteRange>,
    parent: Option<ByteRange>,
    file: usize,
    ckind: CompletionItemKind,
    skind: SymbolKind,
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
        idx >= self.start && idx < self.end
    }
    fn contains_range(&self, range: Self) -> bool {
        range.start >= self.start && range.start < self.end
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

#[derive(Debug, Clone, Copy)]
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
            scope_node: self.scope_node,
            parent: self.parent,
            file: self.file,
            ckind: self.ckind?,
            skind: self.skind?,
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
        if let PortDirection::Interface((i, _)) = &self.direction {
            self.direction = PortDirection::Interface((*i, Some(modport)))
        }
    }
}

struct RopeChunks<'a>(ropey::iter::Chunks<'a>);

impl<'a> RopeChunks<'a> {
    fn new(chunks: ropey::iter::Chunks<'a>) -> Self {
        Self(chunks)
    }
}

impl<'a> Iterator for RopeChunks<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|s| s.as_bytes())
    }
}

pub fn parse(text: &Rope) -> Option<Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(tree_sitter_verilog::language()).ok()?;
    parser.parse_with(
        &mut |offset: usize, pos: Point| {
            let (chunk, chunk_byte_idx, _, _) = text.chunk_at_byte(offset);
            &chunk.as_bytes()[(offset - chunk_byte_idx)..]
        },
        None,
    )
}

pub fn index(text: &Rope, tree: &Tree, query: &Query) -> Vec<Symbol> {
    let mut symbols: Vec<Symbol> = Vec::new();
    let mut struct_members: Vec<SymbolBuilder> = Vec::new();
    let mut cursor = QueryCursor::new();
    let mats = cursor.matches(query, tree.root_node(), |node: Node| {
        RopeChunks::new(text.slice(node.byte_range()).chunks())
    });
    let mut parent: Option<(ByteRange, ByteRange)> = None;
    for m in mats {
        // scopes
        if m.pattern_index == 0 {
            println!("new scope {}", symbols.len());
            let mut builder = SymbolBuilder::new(0);
            for cap in m.captures {
                match query.capture_names()[cap.index as usize].as_str() {
                    "ident" => builder.ident_node(cap.node.byte_range().into()),
                    "keyword" => {
                        builder.type_node(cap.node.byte_range().into());
                        builder.kind(
                            match text.slice(cap.node.byte_range()).to_string().as_str() {
                                "module" | "macromodule" | "primitive" | "program" | "checker" => {
                                    Some((CompletionItemKind::Module, SymbolKind::Module))
                                }
                                "interface" => {
                                    Some((CompletionItemKind::Interface, SymbolKind::Interface))
                                }
                                "task" => {
                                    Some((CompletionItemKind::Function, SymbolKind::Function))
                                }
                                "function" => {
                                    Some((CompletionItemKind::Function, SymbolKind::Function))
                                }
                                "package" => {
                                    Some((CompletionItemKind::Module, SymbolKind::Package))
                                }
                                "class" => Some((CompletionItemKind::Class, SymbolKind::Class)),
                                "struct" => Some((CompletionItemKind::Struct, SymbolKind::Struct)),
                                "union" => Some((CompletionItemKind::Enum, SymbolKind::Enum)),
                                "enum" => Some((CompletionItemKind::Enum, SymbolKind::Enum)),
                                _ => None,
                            },
                        );
                    }
                    "scope" => builder.scope_node(cap.node.byte_range().into()),
                    _ => (),
                }
            }
            let symbol = builder.build();
            if let Some(s) = symbol.as_ref() {
                if let Some(scope) = s.scope_node {
                    parent = Some((s.ident_node, scope));
                }
            }
            symbols.extend(symbol);

            // set struct member parent to struct
            while let Some(mut s) = struct_members.pop() {
                if let (Some(ident), Some(sym)) = (s.ident_node, symbol) {
                    if let Some(scope) = sym.scope_node {
                        if scope.contains_range(ident) {
                            s.parent(sym.ident_node);
                            symbols.extend(s.build());
                            continue;
                        }
                    }
                }
                struct_members.push(s);
            }
        }
        // ports
        else if m.pattern_index == 1 {
            println!("new port");
            let mut builder = SymbolBuilder::new(0);
            builder.direction("inout");
            for cap in m.captures {
                match query.capture_names()[cap.index as usize].as_str() {
                    "ident" => {
                        builder.ident_node(cap.node.byte_range().into());
                        if let Some((pi, pr)) = parent {
                            if pr.contains(cap.node.start_byte()) {
                                builder.parent(pi);
                            }
                        }
                    }
                    "port" => builder.scope_node(cap.node.byte_range().into()),
                    "type" => builder.type_node(cap.node.byte_range().into()),
                    "direction" => {
                        builder.direction(text.slice(cap.node.byte_range()).to_string().as_str())
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
        // params
        else if m.pattern_index == 2 {
            println!("new param");
            let mut builder = SymbolBuilder::new(0);
            for cap in m.captures {
                match query.capture_names()[cap.index as usize].as_str() {
                    "ident" => {
                        builder.ident_node(cap.node.byte_range().into());
                        if let Some((pi, pr)) = parent {
                            if pr.contains(cap.node.start_byte()) {
                                builder.parent(pi);
                            }
                        }
                    }
                    "type" => builder.type_node(cap.node.byte_range().into()),
                    "signed" => builder.signed(),
                    _ => (),
                }
            }
            builder.kind(Some((CompletionItemKind::Property, SymbolKind::Property)));
            symbols.extend(builder.build());
        }
        // package import
        else if m.pattern_index == 3 {
            println!("new member {}", symbols.len());
            let mut builder = SymbolBuilder::new(0);
            for cap in m.captures {
                match query.capture_names()[cap.index as usize].as_str() {
                    "ident" => {
                        builder.ident_node(cap.node.byte_range().into());
                        if let Some((pi, pr)) = parent {
                            if pr.contains(cap.node.start_byte()) {
                                builder.parent(pi);
                            }
                        }
                    }
                    "package" => builder.type_node(cap.node.byte_range().into()),
                    _ => (),
                }
            }
            builder.kind(Some((CompletionItemKind::Property, SymbolKind::Property)));
            symbols.extend(builder.build());
        }
        // struct_union member
        else if m.pattern_index == 4 {
            println!("new member {}", symbols.len());
            let mut builder = SymbolBuilder::new(0);
            for cap in m.captures {
                match query.capture_names()[cap.index as usize].as_str() {
                    "ident" => {
                        builder.ident_node(cap.node.byte_range().into());
                        if let Some((pi, pr)) = parent {
                            if pr.contains(cap.node.start_byte()) {
                                builder.parent(pi);
                            }
                        }
                    }
                    "type" => builder.type_node(cap.node.byte_range().into()),
                    "signed" => builder.signed(),
                    _ => (),
                }
            }
            builder.kind(Some((CompletionItemKind::Field, SymbolKind::Field)));
            struct_members.push(builder);
        }
        // instantiation
        else if m.pattern_index == 5 {
            println!("new member {}", symbols.len());
            let mut builder = SymbolBuilder::new(0);
            for cap in m.captures {
                match query.capture_names()[cap.index as usize].as_str() {
                    "ident" => {
                        builder.ident_node(cap.node.byte_range().into());
                        if let Some((pi, pr)) = parent {
                            if pr.contains(cap.node.start_byte()) {
                                builder.parent(pi);
                            }
                        }
                    }
                    "type" => builder.type_node(cap.node.byte_range().into()),
                    _ => (),
                }
            }
            builder.kind(Some((CompletionItemKind::Module, SymbolKind::Module)));
            symbols.extend(builder.build());
        }
        // variable
        else if m.pattern_index == 6 {
            println!("new member {}", symbols.len());
            let mut builder = SymbolBuilder::new(0);
            for cap in m.captures {
                match query.capture_names()[cap.index as usize].as_str() {
                    "ident" => {
                        builder.ident_node(cap.node.byte_range().into());
                        if let Some((pi, pr)) = parent {
                            if pr.contains(cap.node.start_byte()) {
                                builder.parent(pi);
                            }
                        }
                    }
                    "type" => builder.type_node(cap.node.byte_range().into()),
                    _ => (),
                }
            }
            builder.kind(Some((CompletionItemKind::Variable, SymbolKind::Variable)));
            symbols.extend(builder.build());
        }
    }
    symbols
}

const SYMBOL_QUERY: &str = include_str!("query.scm");

#[cfg(test)]
mod tests {
    use super::*;

    fn test_index(text: &str) -> Vec<Symbol> {
        let rope = Rope::from(text);
        let tree = parse(&rope).unwrap();
        let query = &Query::new(tree_sitter_verilog::language(), SYMBOL_QUERY).unwrap();
        let symbols = index(&rope, &tree, &query);
        for symbol in &symbols {
            println!("{}", range_text(symbol.ident_node, text));
        }
        symbols
    }

    fn port(
        ansi: bool,
        query: &Query,
        port_str: &str,
        direction: &str,
        signed: bool,
        type_str: &str,
        ident_str: &str,
        parent_str: &str,
    ) {
        let text = if ansi {
            format!(
                "
interface a;
    modport b (input c);
endinterface
module test ({});
endmodule
",
                port_str
            )
        } else {
            format!(
                "
interface a;
    modport b (input c);
endinterface
module test ();
    {}
endmodule
",
                port_str
            )
        };
        let mut ind = test_index(&text);
        ind.retain(|s| s.is_port());
        let port = ind.get(0).unwrap();
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

    fn range_text(range: ByteRange, text: &str) -> &str {
        &text[range.start..range.end]
    }

    fn check_symbol(
        text: &str,
        symbols: &Vec<Symbol>,
        ident: &str,
        type_str: &str,
        parent: &str,
        ckind: CompletionItemKind,
    ) {
        let mut found = false;
        for symbol in symbols {
            if (range_text(symbol.ident_node, text) == ident) {
                if let Some(type_node) = symbol.type_node {
                    assert_eq!(range_text(type_node, &text), type_str);
                } else {
                    assert_eq!("", type_str);
                }
                if let Some(parent_node) = symbol.parent {
                    assert_eq!(range_text(parent_node, &text), parent);
                } else {
                    assert_eq!("", parent);
                }
                assert_eq!(symbol.ckind, ckind);
                found = true;
                break;
            }
        }
        assert!(found, "symbol `{}` not found", ident);
    }

    fn check_port(
        text: &str,
        symbols: &Vec<Symbol>,
        ident: &str,
        direction: PortDirection,
        type_str: &str,
        parent: &str,
        ckind: CompletionItemKind,
    ) {
        let mut found = false;
        for symbol in symbols {
            if (range_text(symbol.ident_node, text) == ident) {
                assert_eq!(symbol.direction, direction);
                if let Some(type_node) = symbol.type_node {
                    assert_eq!(range_text(type_node, &text), type_str);
                } else {
                    assert_eq!("", type_str);
                }
                if let Some(parent_node) = symbol.parent {
                    assert_eq!(range_text(parent_node, &text), parent);
                } else {
                    assert_eq!("", parent);
                }
                found = true;
                break;
            }
        }
        assert!(found);
    }

    #[test]
    fn ansi_ports() {
        let query = &Query::new(tree_sitter_verilog::language(), SYMBOL_QUERY).unwrap();
        port(true, query, "wire x", "inout", false, "wire", "x", "test");
        port(
            true,
            query,
            "integer x",
            "inout",
            false,
            "integer",
            "x",
            "test",
        );
        port(
            true,
            query,
            "inout integer x",
            "inout",
            false,
            "integer",
            "x",
            "test",
        );
        port(true, query, "[5:0] x", "inout", false, "", "x", "test");
        port(true, query, "input x", "input", false, "", "x", "test");
        port(
            true,
            query,
            "input var x",
            "input",
            false,
            "var",
            "x",
            "test",
        );
        port(
            true,
            query,
            "input var integer x",
            "input",
            false,
            "var",
            "x",
            "test",
        );
        port(true, query, "output x", "output", false, "", "x", "test");
        port(
            true,
            query,
            "output var x",
            "output",
            false,
            "var",
            "x",
            "test",
        );
        port(
            true,
            query,
            "output integer x",
            "output",
            false,
            "integer",
            "x",
            "test",
        );
        port(true, query, "ref [5:0] x", "ref", false, "", "x", "test");
        port(true, query, "ref x [5:0]", "ref", false, "", "x", "test");
    }

    #[test]
    fn non_ansi_ports() {
        let query = &Query::new(tree_sitter_verilog::language(), SYMBOL_QUERY).unwrap();
        port(
            false,
            query,
            "input [7:0] a;",
            "input",
            false,
            "",
            "a",
            "test",
        );
        port(
            false,
            query,
            "input signed [7:0] b,c,d;",
            "input",
            true,
            "",
            "b",
            "test",
        );
        port(
            false,
            query,
            "output signed f,g;",
            "output",
            true,
            "",
            "f",
            "test",
        );
    }

    #[test]
    fn udp() {
        let text = r#"
primitive multiplexera(mux, control, dataA, dataB);
  output mux;
  input control, dataA, dataB;
  table
    0 1 0 : 1 ;
  endtable
endprimitive
"#;
        let symbols = test_index(text);
        check_symbol(
            text,
            &symbols,
            "multiplexera",
            "primitive",
            "",
            CompletionItemKind::Module,
        );
        check_port(
            text,
            &symbols,
            "mux",
            PortDirection::Output,
            "",
            "multiplexera",
            CompletionItemKind::Module,
        );
        check_port(
            text,
            &symbols,
            "dataA",
            PortDirection::Input,
            "",
            "multiplexera",
            CompletionItemKind::Module,
        );
        let text = r#"
primitive multiplexerb(output mux, input control, input dataA, input dataB);
  table
    0 1 0 : 1 ;
  endtable
endprimitive
"#;
        let symbols = test_index(text);
        check_symbol(
            text,
            &symbols,
            "multiplexerb",
            "primitive",
            "",
            CompletionItemKind::Module,
        );
        check_port(
            text,
            &symbols,
            "mux",
            PortDirection::Output,
            "",
            "multiplexerb",
            CompletionItemKind::Module,
        );
        check_port(
            text,
            &symbols,
            "dataA",
            PortDirection::Input,
            "",
            "multiplexerb",
            CompletionItemKind::Module,
        );
    }

    #[test]
    fn struct_union_enum() {
        let text = r#"
struct {
  bit [7:0]  opcode;
  bit [23:0] addr;
} IR1;

typedef struct {
  bit [7:0]  opcode;
  bit [23:0] addr;
} instruction;

enum {red, yellow, green} light1, light2;

typedef union { int i; shortreal f; } num;
"#;
        let symbols = test_index(text);
        check_symbol(
            text,
            &symbols,
            "IR1",
            "struct",
            "",
            CompletionItemKind::Struct,
        );
        check_symbol(
            text,
            &symbols,
            "opcode",
            "bit [7:0]",
            "IR1",
            CompletionItemKind::Field,
        );
        check_symbol(
            text,
            &symbols,
            "instruction",
            "struct",
            "",
            CompletionItemKind::Struct,
        );
        check_symbol(
            text,
            &symbols,
            "light2",
            "enum",
            "",
            CompletionItemKind::Enum,
        );
        check_symbol(text, &symbols, "num", "union", "", CompletionItemKind::Enum);
    }

    #[test]
    fn params() {
        let text = r#"
parameter logic [7:0] My_DataIn = 8'hFF;

module generic_fifo1;
    parameter MSB=3;
endmodule

module generic_fifo #(parameter MSB=3, LSB=0, DEPTH=4) ();
endmodule

module generic_decoder #(num_code_bits = 3, localparam num_out_bits = 1 << num_code_bits, ABC=1)
(input [num_code_bits-1:0] A, output reg [num_out_bits-1:0] Y);
endmodule

extern module a #(parameter size= 8, parameter type TP = logic [7:0])
(input [size:0] a, output TP b);
"#;
        let symbols = test_index(text);
        check_symbol(
            text,
            &symbols,
            "My_DataIn",
            "parameter",
            "",
            CompletionItemKind::Property,
        );
        check_symbol(
            text,
            &symbols,
            "MSB",
            "parameter",
            "generic_fifo1",
            CompletionItemKind::Property,
        );
        check_symbol(
            text,
            &symbols,
            "LSB",
            "",
            "generic_fifo",
            CompletionItemKind::Property,
        );
        check_symbol(
            text,
            &symbols,
            "num_out_bits",
            "localparam",
            "generic_decoder",
            CompletionItemKind::Property,
        );
    }

    #[test]
    fn tf() {
        let text = r#"
task mytask1 (output int x, input logic y);
endtask

task mytask3(a, b, output logic [15:0] u, v);
endtask

task mytask2;
    output f;
    input g;
    int f;
    logic g;
endtask

function logic [15:0] myfunc1(int h, int i);
endfunction

function logic [15:0] myfunc2;
    input int j;
    input int k;
endfunction

function [3:0][7:0] myfunc4(input [3:0][7:0] p, q[3:0]);
endfunction
"#;
        let symbols = test_index(text);
        check_symbol(
            text,
            &symbols,
            "mytask1",
            "task",
            "",
            CompletionItemKind::Function,
        );
        check_symbol(
            text,
            &symbols,
            "x",
            "int",
            "mytask1",
            CompletionItemKind::Property,
        );
        check_symbol(
            text,
            &symbols,
            "y",
            "logic",
            "mytask1",
            CompletionItemKind::Property,
        );
        check_symbol(
            text,
            &symbols,
            "mytask3",
            "task",
            "",
            CompletionItemKind::Function,
        );
        check_symbol(
            text,
            &symbols,
            "a",
            "",
            "mytask3",
            CompletionItemKind::Property,
        );
        check_symbol(
            text,
            &symbols,
            "u",
            "logic [15:0]",
            "mytask3",
            CompletionItemKind::Property,
        );
        check_symbol(
            text,
            &symbols,
            "mytask2",
            "task",
            "",
            CompletionItemKind::Function,
        );
        check_symbol(
            text,
            &symbols,
            "f",
            "",
            "mytask2",
            CompletionItemKind::Property,
        );
        check_symbol(
            text,
            &symbols,
            "myfunc4",
            "function",
            "",
            CompletionItemKind::Function,
        );
        check_symbol(
            text,
            &symbols,
            "p",
            "",
            "myfunc4",
            CompletionItemKind::Property,
        );
        check_symbol(
            text,
            &symbols,
            "q",
            "q[3:0]",
            "myfunc4",
            CompletionItemKind::Property,
        );
        check_symbol(
            text,
            &symbols,
            "myfunc2",
            "function",
            "",
            CompletionItemKind::Function,
        );
        check_symbol(
            text,
            &symbols,
            "j",
            "",
            "myfunc2",
            CompletionItemKind::Property,
        );
        check_symbol(
            text,
            &symbols,
            "k",
            "",
            "myfunc2",
            CompletionItemKind::Property,
        );
    }

    #[test]
    fn import() {
        let text = r#"
module top1 ;
    import p::*;
    import q::teeth_t;
    teeth_t myteeth;
    initial begin
        myteeth = q:: FALSE;
        myteeth = FALSE;
    end
endmodule

module top2 ;
    import p::*;
    import q::teeth_t, q::ORIGINAL, q::FALSE;
    teeth_t myteeth;
    initial begin
        myteeth = FALSE;
    end
endmodule
"#;
        let symbols = test_index(text);
        check_symbol(
            text,
            &symbols,
            "*",
            "p",
            "top1",
            CompletionItemKind::Property,
        );
        check_symbol(
            text,
            &symbols,
            "ORIGINAL",
            "q",
            "top2",
            CompletionItemKind::Property,
        );
    }

    #[test]
    fn instantiation() {
        let text = r#"
module alu_accum1;
  alu alu1 (
      alu_out,
      ain,
      bin,
      opcode
  );  // zero output is unconnected
  accum accum (
      dataout[7:0],
      alu_out,
      clk,
      rst_n
  );
  xtend xtend3 (
      .dout(dataout[15:8]),
      .din(alu_out[7]),
      .clk(clk)
  );
endmodule
"#;
        let symbols = test_index(text);
        check_symbol(
            text,
            &symbols,
            "alu1",
            "alu",
            "alu_accum1",
            CompletionItemKind::Module,
        );
        check_symbol(
            text,
            &symbols,
            "accum",
            "accum",
            "alu_accum1",
            CompletionItemKind::Module,
        );
        check_symbol(
            text,
            &symbols,
            "xtend3",
            "xtend",
            "alu_accum1",
            CompletionItemKind::Module,
        );
    }

    #[test]
    fn scope() {
        let text = r#"
module test1;
endmodule

program test2;
endprogram

interface test3;
endinterface

interface test4(clk);
endinterface

checker test5;
    covergroup group;
    endgroup
endchecker

package test6;
endpackage

class test7;
endclass
    "#;
        let symbols = test_index(text);
        check_symbol(
            text,
            &symbols,
            "test1",
            "module",
            "",
            CompletionItemKind::Module,
        );
        check_symbol(
            text,
            &symbols,
            "test2",
            "program",
            "",
            CompletionItemKind::Module,
        );
        check_symbol(
            text,
            &symbols,
            "test3",
            "interface",
            "",
            CompletionItemKind::Interface,
        );
        check_symbol(
            text,
            &symbols,
            "test4",
            "interface",
            "",
            CompletionItemKind::Interface,
        );
        check_symbol(
            text,
            &symbols,
            "test5",
            "checker",
            "",
            CompletionItemKind::Module,
        );
        check_symbol(
            text,
            &symbols,
            "test6",
            "package",
            "",
            CompletionItemKind::Module,
        );
        check_symbol(
            text,
            &symbols,
            "test7",
            "class",
            "",
            CompletionItemKind::Class,
        );
    }

    #[test]
    fn net_dec() {
        let t = r#"
module test;
    wire a;
    wand b;
    wor c;

    tri d;
    triand e;
    trior f;

    tri0 g;
    tri1 h;
    trireg (small) signed [3:0] cap2;

    supply0 j;
    supply1 k;
    uwire l;

    nettype T wTsum with Tsum;
    typedef real TR[5];

    trireg (large) logic #(0,0,0) cap1;
    typedef logic [31:0] addressT;
    wire addressT w1;
    wire struct packed { logic ecc; logic [7:0] data; } memsig;
endmodule
    "#;
        let s = test_index(t);
        check_symbol(t, &s, "a", "wire", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "b", "wand", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "c", "wor", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "d", "tri", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "e", "triand", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "f", "trior", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "g", "tri0", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "h", "tri1", "test", CompletionItemKind::Variable);
        check_symbol(
            t,
            &s,
            "cap2",
            "trireg",
            "test",
            CompletionItemKind::Variable,
        );
        check_symbol(t, &s, "j", "supply0", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "k", "supply1", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "l", "uwire", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "wTsum", "T", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "TR", "real", "test", CompletionItemKind::Variable);
        check_symbol(
            t,
            &s,
            "cap1",
            "trireg",
            "test",
            CompletionItemKind::Variable,
        );
        check_symbol(
            t,
            &s,
            "addressT",
            "logic",
            "test",
            CompletionItemKind::Variable,
        );
        check_symbol(t, &s, "w1", "wire", "test", CompletionItemKind::Variable);
        check_symbol(
            t,
            &s,
            "memsig",
            "wire",
            "test",
            CompletionItemKind::Variable,
        );
    }

    #[test]
    fn data_dec() {
        let t = r#"
module test;
    var byte a;
    int b;
    shortint c;
    longint d;
    bit e;
    reg f;
    integer g;
    time h;
    logic [1:0] i [1:0];
    logic [4:0] j, k;
    real l;
    shortreal m;
    string n = "";
    chandle o;
    var p;

    typedef logic [15:0] r_t;
endmodule
    "#;
        let s = test_index(t);
        check_symbol(t, &s, "a", "byte", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "b", "int", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "c", "shortint", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "d", "longint", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "e", "bit", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "f", "reg", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "g", "integer", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "h", "time", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "i", "logic", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "k", "logic", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "l", "real", "test", CompletionItemKind::Variable);
        check_symbol(
            t,
            &s,
            "m",
            "shortreal",
            "test",
            CompletionItemKind::Variable,
        );
        check_symbol(t, &s, "n", "string", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "o", "chandle", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "r_t", "logic", "test", CompletionItemKind::Variable);
        check_symbol(t, &s, "p", "var", "test", CompletionItemKind::Variable);
    }
}
