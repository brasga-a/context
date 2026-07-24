use std::sync::{Arc, RwLock};

use context_engine::{EditError, RetrievalError, VaultIndex};
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

#[derive(Debug, Deserialize, JsonSchema)]
struct EditSectionParameters {
    /// Vault-relative Markdown file path.
    file: String,
    /// Exact disambiguated heading breadcrumb, such as `Skills > Gun`.
    heading_path: String,
    /// New Markdown body for the section; the heading line is preserved as-is.
    body: String,
    /// The section's `content_hash` from a prior read; the edit fails on mismatch.
    expected_hash: String,
}

#[derive(Clone)]
struct ContextServer {
    index: Arc<RwLock<VaultIndex>>,
}

#[tool_router]
impl ContextServer {
    fn new(index: VaultIndex) -> Self {
        Self {
            index: Arc::new(RwLock::new(index)),
        }
    }

    fn read_index(&self) -> std::sync::RwLockReadGuard<'_, VaultIndex> {
        self.index.read().expect("vault index lock poisoned")
    }

    #[tool(description = "Return a Markdown document's section outline without body text")]
    fn outline(
        &self,
        Parameters(OutlineParameters { file }): Parameters<OutlineParameters>,
    ) -> CallToolResult {
        match self.read_index().outline(&file) {
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
        match self.read_index().get_section(&file, &heading_path) {
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
            "results": self.read_index().search(&query),
        }))
    }

    #[tool(
        description = "Replace one section's body (heading preserved), guarded by its content_hash from a prior read"
    )]
    fn edit_section(
        &self,
        Parameters(EditSectionParameters {
            file,
            heading_path,
            body,
            expected_hash,
        }): Parameters<EditSectionParameters>,
    ) -> CallToolResult {
        let mut index = self.index.write().expect("vault index lock poisoned");
        match index.edit_section(&file, &heading_path, &body, &expected_hash) {
            Ok(sections) => CallToolResult::structured(json!({
                "file": file,
                "sections": sections,
            })),
            Err(error) => edit_error(error),
        }
    }
}

#[tool_handler(
    name = "context",
    version = "0.1.0",
    instructions = "Retrieve and edit exact, span-backed sections of the indexed Markdown vault."
)]
impl ServerHandler for ContextServer {}

fn retrieval_error(error: RetrievalError) -> CallToolResult {
    CallToolResult::structured_error(json!({
        "message": error.message,
        "suggestions": error.suggestions,
    }))
}

fn edit_error(error: EditError) -> CallToolResult {
    match error {
        EditError::NotFound(error) => retrieval_error(error),
        EditError::Conflict {
            message,
            current_hash,
        } => CallToolResult::structured_error(json!({
            "message": message,
            "current_hash": current_hash,
        })),
        other => CallToolResult::structured_error(json!({
            "message": other.message(),
        })),
    }
}

pub(crate) async fn serve(index: VaultIndex) -> Result<(), Box<dyn std::error::Error>> {
    let service = ContextServer::new(index).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
