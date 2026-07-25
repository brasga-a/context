//! Structural indexing and retrieval for source-backed Markdown documents.

mod diff;
mod document;
mod edit;
mod links;
mod section;
mod vault;

pub use diff::{FileDiff, VaultDiff};
pub use document::{DocumentDiagnostic, EngineDocument};
pub use edit::{EditError, HashedOutlineSection};
pub use links::Backlink;
pub use section::{Section, build_section_tree};
pub use vault::{
    ByteRange, LineRange, OutlineSection, Provenance, RetrievalError, RetrievedSection,
    SearchResult, VaultDiagnostic, VaultError, VaultIndex,
};
