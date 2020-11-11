/* --------------------------------------------------------------------------------------------
 * Copyright (c) Microsoft Corporation. All rights reserved.
 * Licensed under the MIT License. See License.txt in the project root for license information.
 * ------------------------------------------------------------------------------------------ */

import { workspace, ExtensionContext } from "vscode";

import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  Executable,
} from "vscode-languageclient";

let client: LanguageClient;
const workSpaceFolder = workspace.workspaceFolders?.[0];
let cwd: string = workSpaceFolder.uri.fsPath;

export function activate(context: ExtensionContext) {
  const run: Executable = {
    command: "veridian",
    // options: { cwd },
  };

  // If the extension is launched in debug mode then the debug server options are used
  // Otherwise the run options are used
  let serverOptions: ServerOptions = {
    run,
    debug: run,
  };

  // Options to control the language client
  let clientOptions: LanguageClientOptions = {
    // Register the server for plain text documents
    documentSelector: [{ scheme: "file", language: "systemverilog" }],
  };

  // Create the language client and start the client.
  client = new LanguageClient(
    "veridian",
    "veridian",
    serverOptions,
    clientOptions
  );

  // Start the client. This will also launch the server
  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
