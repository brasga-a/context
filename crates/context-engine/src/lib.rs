//! Structural indexing and retrieval for source-backed Markdown documents.

mod document;
mod section;
mod vault;

pub use document::{DocumentDiagnostic, EngineDocument};
pub use section::{Section, build_section_tree};
pub use vault::{
    ByteRange, LineRange, OutlineSection, Provenance, RetrievalError, RetrievedSection,
    SearchResult, VaultDiagnostic, VaultError, VaultIndex,
};
