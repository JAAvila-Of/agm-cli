// crates/agm-lsp/src/main.rs

use agm_lsp::backend;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(backend::AgmLanguageServer::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}
