use context_parser::{Span, parse};

use crate::{Section, build_section_tree};

/// A non-fatal problem encountered while interpreting one Markdown document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentDiagnostic {
    /// Source range associated with the problem, when one exists.
    pub span: Option<Span>,
    /// Human-readable diagnostic suitable for logs and tool errors.
    pub message: String,
}

/// An owned Markdown document with interpreted metadata and derived sections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineDocument {
    source: String,
    /// YAML frontmatter, when present, terminated, and valid.
    pub metadata: Option<yaml_serde::Value>,
    /// Root sections in document order.
    pub sections: Vec<Section>,
    /// Non-fatal interpretation diagnostics.
    pub diagnostics: Vec<DocumentDiagnostic>,
}

impl EngineDocument {
    /// Parses and structurally indexes one Markdown source string.
    pub fn parse(source: impl Into<String>) -> Self {
        let source = source.into();
        let parsed = parse(&source);
        let sections = build_section_tree(&source, &parsed);
        let mut diagnostics = Vec::new();

        let metadata = parsed
            .frontmatter
            .filter(|frontmatter| frontmatter.terminated)
            .and_then(|frontmatter| {
                let yaml = frontmatter_yaml(frontmatter.span.slice(&source));
                match yaml_serde::from_str(yaml) {
                    Ok(value) => Some(value),
                    Err(error) => {
                        diagnostics.push(DocumentDiagnostic {
                            span: Some(frontmatter.span),
                            message: format!("invalid YAML frontmatter: {error}"),
                        });
                        None
                    }
                }
            });

        Self {
            source,
            metadata,
            sections,
            diagnostics,
        }
    }

    /// Returns the exact source text backing all section spans.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
}

fn frontmatter_yaml(frontmatter: &str) -> &str {
    let frontmatter = frontmatter.strip_prefix('\u{feff}').unwrap_or(frontmatter);
    let Some(opening_end) = frontmatter.find('\n') else {
        return "";
    };
    let through_closing = frontmatter.trim_end_matches(['\r', '\n']);
    let closing_start = through_closing
        .rfind('\n')
        .map_or(opening_end + 1, |position| position + 1);
    let yaml = &frontmatter[opening_end + 1..closing_start];
    yaml.strip_suffix('\r').unwrap_or(yaml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_frontmatter_becomes_document_metadata() {
        let document = EngineDocument::parse(
            "---
tags: [character]
level: 3
---
# Player
body",
        );

        let metadata = document.metadata.as_ref().expect("valid metadata");
        assert_eq!(metadata["tags"][0].as_str(), Some("character"));
        assert_eq!(metadata["level"].as_i64(), Some(3));
        assert!(document.diagnostics.is_empty());
        assert_eq!(document.sections[0].heading_path, "Player");
    }

    #[test]
    fn invalid_frontmatter_is_non_fatal_and_diagnostic() {
        let document = EngineDocument::parse(
            "---
tags: [broken
---
# Still indexed
body",
        );

        assert!(document.metadata.is_none());
        assert_eq!(document.diagnostics.len(), 1);
        assert!(
            document.diagnostics[0]
                .message
                .contains("invalid YAML frontmatter")
        );
        assert_eq!(document.sections[0].heading_path, "Still indexed");
    }

    #[test]
    fn absent_frontmatter_produces_no_metadata_or_diagnostic() {
        let document = EngineDocument::parse("# Plain\nbody");

        assert!(document.metadata.is_none());
        assert!(document.diagnostics.is_empty());
        assert_eq!(document.sections.len(), 1);
    }
}
