use std::sync::Arc;

use context_engine::{RetrievalError, VaultIndex};
use rmcp::{
    ServerHandler, ServiceExt, handler::server::wrapper::Parameters, model::CallToolResult, tool,
    tool_handler, tool_router, transport::stdio,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize, JsonSchema)]
struct OutlineParameters {
    /// Vault-relative Markdown file path.
    file: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GetSectionParameters {
    /// Vault-relative Markdown file path.
    file: String,
    /// Exact disambiguated heading breadcrumb, such as `Skills > Gun`.
    heading_path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchParameters {
    /// Free-text heading or heading-path query.
    query: String,
}

#[derive(Clone)]
struct ContextServer {
    index: Arc<VaultIndex>,
}

#[tool_router]
impl ContextServer {
    fn new(index: VaultIndex) -> Self {
        Self {
            index: Arc::new(index),
        }
    }

    #[tool(description = "Return a Markdown document's section outline without body text")]
    fn outline(
        &self,
        Parameters(OutlineParameters { file }): Parameters<OutlineParameters>,
    ) -> CallToolResult {
        match self.index.outline(&file) {
            Ok(sections) => CallToolResult::structured(json!({
                "file": file,
                "sections": sections,
            })),
            Err(error) => retrieval_error(error),
        }
    }

    #[tool(description = "Return one byte-exact Markdown section with source provenance")]
    fn get_section(
        &self,
        Parameters(GetSectionParameters { file, heading_path }): Parameters<GetSectionParameters>,
    ) -> CallToolResult {
        match self.index.get_section(&file, &heading_path) {
            Ok(section) => CallToolResult::structured(json!(section)),
            Err(error) => retrieval_error(error),
        }
    }

    #[tool(description = "Search headings and heading paths across the indexed Markdown vault")]
    fn search(
        &self,
        Parameters(SearchParameters { query }): Parameters<SearchParameters>,
    ) -> CallToolResult {
        CallToolResult::structured(json!({
            "query": query,
            "results": self.index.search(&query),
        }))
    }
}

#[tool_handler(
    name = "context",
    version = "0.1.0",
    instructions = "Retrieve exact, span-backed sections from the indexed Markdown vault."
)]
impl ServerHandler for ContextServer {}

fn retrieval_error(error: RetrievalError) -> CallToolResult {
    CallToolResult::structured_error(json!({
        "message": error.message,
        "suggestions": error.suggestions,
    }))
}

pub(crate) async fn serve(index: VaultIndex) -> Result<(), Box<dyn std::error::Error>> {
    let service = ContextServer::new(index).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
