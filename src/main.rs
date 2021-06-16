#[allow(unused)]
mod symbol;

use log::info;
use std::sync::Arc;
use structopt::StructOpt;
use tower_lsp::{LspService, Server};

mod completion;
mod definition;
mod diagnostics;
mod format;
mod server;
mod sources;
#[cfg(test)]
mod support;
use server::Backend;

#[derive(StructOpt, Debug)]
#[structopt(name = "veridian", about = "A SystemVerilog/Verilog Language Server")]
struct Opt {}

#[tokio::main]
async fn main() {
    let _ = Opt::from_args();
    let log_handle = flexi_logger::Logger::with(flexi_logger::LogSpecification::info())
        .start()
        .unwrap();
    info!("starting veridian...");

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, messages) = LspService::new(|client| Arc::new(Backend::new(client, log_handle)));
    Server::new(stdin, stdout, messages).serve(service).await;
}
