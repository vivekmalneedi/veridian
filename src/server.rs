use crate::sources::*;

use log::info;
use lsp_server::{Connection, Message, Notification, Request, RequestId};
use lsp_types::{notification::*, request::*, InitializeParams};
use notify::{RecommendedWatcher, Watcher};
use std::error::Error;

pub struct LSPServer {
    pub srcs: Sources,
    pub watcher: RecommendedWatcher,
}

impl LSPServer {
    pub fn new() -> LSPServer {
        LSPServer {
            srcs: Sources::new(),
            watcher: Watcher::new_immediate(|res| match res {
                Ok(event) => (),
                Err(e) => println!("watch error: {:?}", e),
            })
            .unwrap(),
        }
    }
}

pub fn main_loop(
    connection: &Connection,
    params: serde_json::Value,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let mut server = LSPServer::new();

    let _params: InitializeParams = serde_json::from_value(params).unwrap();
    info!("starting server loop");
    for msg in &connection.receiver {
        info!("got msg: {:?}", msg);
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                info!("got request: {:?}", req);
                match cast_request::<Completion>(req) {
                    Ok((id, params)) => {
                        info!("got completion request #{}: {:?}", id, params);
                        connection
                            .sender
                            .send(Message::Response(server.completion(id, params)))?;
                        continue;
                    }
                    Err(req) => req,
                };
                // ...
            }
            Message::Response(resp) => {
                info!("got response: {:?}", resp);
            }
            Message::Notification(not) => {
                info!("got notification: {:?}", not);
                match cast_notification::<DidOpenTextDocument>(not) {
                    Ok(params) => {
                        info!("handling didOpen");
                        server.did_open(params);
                        continue;
                    }
                    Err(not) => not,
                };
                /*
                match cast_notification::<DidChangeTextDocument>(not) {
                    Ok(params) => {
                        info!("handling change");
                        continue;
                    }
                    Err(not) => not,
                };
                */
            }
        }
    }
    Ok(())
}

fn cast_request<R>(req: Request) -> Result<(RequestId, R::Params), Request>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}

fn cast_notification<N>(not: Notification) -> Result<N::Params, Notification>
where
    N: lsp_types::notification::Notification,
    N::Params: serde::de::DeserializeOwned,
{
    not.extract(N::METHOD)
}
