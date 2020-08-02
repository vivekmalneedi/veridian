use lsp_types::*;
use verilogls::server;

#[test]
fn test_completion() {
    let io = server::init();

    let doc = r#"module test;
    logic abc;
    logic abcd;
endmodule
"#;
    let url = Url::parse("file:///doc.sv").unwrap();
    let params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: url.clone(),
            language_id: "system_verilog".to_owned(),
            version: 1,
            text: doc.to_owned(),
        },
    };
    let request = Notification {
        jsonrpc: Some(Version::V2),
        method: "textDocument/didOpen".to_owned(),
        params: Params::Map(to_value(params).unwrap().as_object().unwrap().to_owned()),
    };
    io.handle_request(&to_string(&request).unwrap())
        .wait()
        .unwrap();

    let pos = TextDocumentPositionParams::new(
        TextDocumentIdentifier::new(url),
        Position::new(1, 12),
    );
    let params = CompletionParams {
        text_document_position: pos,
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: None,
        },
        context: None,
    };
    let request = MethodCall {
        jsonrpc: Some(Version::V2),
        method: "textDocument/completion".to_string(),
        params: Params::Map(to_value(params).unwrap().as_object().unwrap().to_owned()),
        id: Id::Num(1),
    };

    let response = io
        .handle_rpc_request(Request::Single(Call::MethodCall(request)))
        .wait()
        .unwrap()
        .unwrap();

    let mut idents: Vec<&str> = Vec::new();
    idents.push("abc");
    idents.push("abcd");

    let expected = CompletionList {
        is_incomplete: true,
        items: idents
            .iter()
            .map(|x| CompletionItem {
                label: (*x).to_owned(),
                kind: None,
                detail: None,
                documentation: None,
                deprecated: None,
                preselect: None,
                sort_text: None,
                filter_text: None,
                insert_text: None,
                insert_text_format: None,
                text_edit: None,
                additional_text_edits: None,
                command: None,
                data: None,
                tags: None,
            })
            .collect(),
    };
    let result = match response {
        Response::Single(x) => match x {
            Output::Success(y) => Some(y.result),
            Output::Failure(_) => None,
        },
        Response::Batch(_) => None,
    }
    .unwrap();

    assert_eq!(result, serde_json::to_string(&expected).unwrap());
}
