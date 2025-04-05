#[allow(unused)]
mod symbol;

use symbol::*;
mod completion;
mod server;
mod sources;
mod support;
mod diagnostics;
mod definition;

// use log::info;
// use std::sync::Arc;
// use structopt::StructOpt;
// use tower_lsp::{LspService, Server};
//
// mod completion;
// mod definition;
// mod diagnostics;
// mod format;
// mod server;
// mod sources;
// #[cfg(test)]
// mod support;
// use server::Backend;
//
// #[derive(StructOpt, Debug)]
// #[structopt(name = "veridian", about = "A SystemVerilog/Verilog Language Server")]
// struct Opt {}

// #[tokio::main]
// async fn main() {
//     let _ = Opt::from_args();
//     let log_handle = flexi_logger::Logger::with(flexi_logger::LogSpecification::info())
//         .start()
//         .unwrap();
//     info!("starting veridian...");
//
//     let stdin = tokio::io::stdin();
//     let stdout = tokio::io::stdout();
//
//     let (service, messages) = LspService::new(|client| Arc::new(Backend::new(client, log_handle)));
//     Server::new(stdin, stdout, messages).serve(service).await;
// }

fn main() {
    let text = "
interface a;
    modport b (input c);
endinterface
module test;
endmodule
module test1; 
    logic itest;
endmodule
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
";
    let syms = test_index(text);
    let tok = "i";
    let pos = 80;
    let mut cand: Vec<Symbol> = Vec::new();
    let mut stack: Vec<Symbol> = Vec::new();
    for sym in syms {
        // pop scope from stack if it doesn't contain sym
        if let Some(scope) = stack.last().and_then(|s| s.scope_node) {
            if !scope.contains(sym.ident_node.start) {
                stack.pop();
            }
        }
        // push scope to stack
        if let Some(scope) = sym.scope_node {
            // multiple definitions can create equivalent scopes
            if let Some(last) = stack.last().and_then(|s| s.scope_node) {
                if scope != last {
                    stack.push(sym);
                }
            } else {
                stack.push(sym);
            }
        }
        // check if parent of sym contains pos
        if sym.parent.is_some() {
            if let Some(scope) = stack.last().and_then(|s| s.scope_node) {
                if !scope.contains(pos) {
                    continue;
                }
            }
        }
        print!("stack: ");
        for sym in &stack {
            print!("{} ", range_text(sym.ident_node, text));
        }
        println!();
        if range_text(sym.ident_node, text).starts_with(tok) {
            cand.push(sym);
        }
    }
    println!("candidates:");
    for sym in cand {
        let parent = match sym.parent {
            Some(p) => range_text(p, text),
            None => "",
        };
        println!(
            "sym: {}, parent: {}",
            range_text(sym.ident_node, text),
            parent
        );
    }
}
