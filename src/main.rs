#![recursion_limit = "256"]

use log::info;
use std::sync::Arc;
use structopt::StructOpt;
use tower_lsp::{LspService, Server};

mod completion;
mod definition;
mod diagnostics;
mod server;
mod sources;
use server::Backend;

#[derive(StructOpt, Debug)]
#[structopt(name = "veridian", about = "A SystemVerilog/Verilog Language Server")]
struct Opt {}

#[tokio::main]
async fn main() {
    let _ = Opt::from_args();
    flexi_logger::Logger::with_str("info").start().unwrap();
    info!("starting LSP server");

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, messages) = LspService::new(|client| Arc::new(Backend::new(client)));
    Server::new(stdin, stdout)
        .interleave(messages)
        .serve(service)
        .await;
}
