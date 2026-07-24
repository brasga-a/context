use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use context_parser::{Block, Span, parse};
use serde::Serialize;

use crate::section::inline_text;
use crate::vault::{
    LineRange, RetrievalError, VaultIndex, find_section, line_range, nearest,
    normalize_request_path, visit_sections,
};
use crate::{EngineDocument, Section};

/// One outline entry of an edited document, carrying its fresh edit-guard hash.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HashedOutlineSection {
    pub heading: String,
    pub level: u8,
    pub heading_path: String,
    pub line_range: LineRange,
    /// BLAKE3 hash of the section's current source bytes — the next edit's guard token.
    pub content_hash: String,
    pub children: Vec<HashedOutlineSection>,
}

/// A failed section edit; no file or index modification occurred.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
    /// The file or heading path does not exist.
    NotFound(RetrievalError),
    /// The section's on-disk bytes no longer match the expected hash.
    Conflict {
        message: String,
        current_hash: String,
    },
    /// The new body contains a heading that would escape the section.
    Escape { message: String },
    /// The addressed section cannot be edited (synthetic preamble).
    InvalidTarget { message: String },
    /// The spliced document would change sections outside the edited one.
    Restructure { message: String },
    /// Reading or writing the vault file failed.
    Io { message: String },
}

impl EditError {
    /// Returns the human-readable message common to all variants.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::NotFound(error) => &error.message,
            Self::Conflict { message, .. }
            | Self::Escape { message }
            | Self::InvalidTarget { message }
            | Self::Restructure { message }
            | Self::Io { message } => message,
        }
    }
}

impl fmt::Display for EditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(error) => error.fmt(formatter),
            _ => formatter.write_str(self.message()),
        }
    }
}

impl Error for EditError {}

impl VaultIndex {
    /// Replaces one section's body on disk and in the index, guarded by its content hash.
    ///
    /// The file is re-read from disk and the hash verified against its current bytes, so a
    /// stale in-memory index can never authorize a bad splice. On success the edited
    /// document's outline is returned with fresh per-section hashes.
    pub fn edit_section(
        &mut self,
        file: &str,
        heading_path: &str,
        body: &str,
        expected_hash: &str,
    ) -> Result<Vec<HashedOutlineSection>, EditError> {
        let Some(key) = normalize_request_path(file) else {
            return Err(EditError::NotFound(RetrievalError {
                message: format!("file '{file}' is not a vault-relative path"),
                suggestions: Vec::new(),
            }));
        };
        let path = vault_file_path(&self.root, &key);
        let source = fs::read_to_string(&path).map_err(|error| {
            EditError::NotFound(RetrievalError {
                message: format!("file '{file}' could not be read: {error}"),
                suggestions: nearest(&key, self.documents.keys().map(String::as_str), 3),
            })
        })?;

        let document = EngineDocument::parse(source);
        let Some(section) = find_section(&document.sections, heading_path) else {
            let mut paths = Vec::new();
            visit_sections(&document.sections, &mut |section| {
                paths.push(section.heading_path.clone());
            });
            return Err(EditError::NotFound(RetrievalError {
                message: format!("heading path '{heading_path}' was not found in file '{key}'"),
                suggestions: nearest(heading_path, paths.iter().map(String::as_str), 3),
            }));
        };
        let Some(heading_span) = section.heading_span else {
            return Err(EditError::InvalidTarget {
                message: format!(
                    "the preamble of '{key}' has no heading and cannot be edited with \
                     edit_section"
                ),
            });
        };
        if section.content_hash != expected_hash {
            return Err(EditError::Conflict {
                message: format!(
                    "section '{heading_path}' in '{key}' changed since it was read: expected \
                     hash {expected_hash}, current hash {}",
                    section.content_hash
                ),
                current_hash: section.content_hash.clone(),
            });
        }

        validate_no_escape(body, section.level, heading_path)?;
        let new_source = splice(document.source(), heading_span, section.span, body);
        let new_document = EngineDocument::parse(new_source);
        verify_skeleton(&document, &new_document, heading_path)?;

        write_atomically(&path, new_document.source()).map_err(|error| EditError::Io {
            message: format!("failed to write '{key}': {error}"),
        })?;
        self.documents.insert(key.clone(), new_document);
        self.rebuild_links();
        let document = &self.documents[&key];
        Ok(hashed_outline(document.source(), &document.sections))
    }

    /// Re-parses one on-disk file into the index, refreshing a possibly stale entry.
    pub fn reindex_file(&mut self, file: &str) -> Result<(), RetrievalError> {
        let Some(key) = normalize_request_path(file) else {
            return Err(RetrievalError {
                message: format!("file '{file}' is not a vault-relative path"),
                suggestions: Vec::new(),
            });
        };
        let path = vault_file_path(&self.root, &key);
        let source = fs::read_to_string(&path).map_err(|error| RetrievalError {
            message: format!("file '{file}' could not be read: {error}"),
            suggestions: nearest(&key, self.documents.keys().map(String::as_str), 3),
        })?;
        self.documents.insert(key, EngineDocument::parse(source));
        self.rebuild_links();
        Ok(())
    }
}

fn vault_file_path(root: &Path, key: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    path.extend(key.split('/'));
    path
}

/// Rejects a body whose top-level headings would terminate the edited section early.
///
/// The body is parsed behind a leading newline so a `---` opener cannot register as
/// frontmatter and hide headings from the check. Headings nested inside container blocks
/// are allowed: the section tree derives only from top-level headings.
fn validate_no_escape(body: &str, level: u8, heading_path: &str) -> Result<(), EditError> {
    let guarded = format!("\n{body}");
    for block in &parse(&guarded).children {
        if let Block::Heading {
            level: found,
            content,
            ..
        } = block
            && *found <= level
        {
            let heading = inline_text(&guarded, content).trim().to_owned();
            return Err(EditError::Escape {
                message: format!(
                    "new body contains heading '{heading}' at level {found}, which would \
                     escape section '{heading_path}' at level {level}; use headings deeper \
                     than level {level} or edit a higher-level section"
                ),
            });
        }
    }
    Ok(())
}

/// Replaces exactly the body byte range; every byte outside it is preserved verbatim.
fn splice(source: &str, heading_span: Span, section_span: Span, body: &str) -> String {
    let heading_end = heading_span.end as usize;
    let section_end = section_span.end as usize;
    let body = body.trim_end();
    if body.is_empty() {
        return format!("{}{}", &source[..heading_end], &source[section_end..]);
    }

    let mut spliced = String::with_capacity(source.len() + body.len());
    spliced.push_str(&source[..heading_end]);
    if source[heading_end..].starts_with("\r\n") {
        spliced.push_str("\r\n");
    } else {
        spliced.push('\n');
    }
    spliced.push_str(body);
    spliced.push_str(&source[section_end..]);
    spliced
}

/// Rejects a splice that changed any section outside the edited one's subtree.
fn verify_skeleton(
    old: &EngineDocument,
    new: &EngineDocument,
    edited_path: &str,
) -> Result<(), EditError> {
    if skeleton(&old.sections, edited_path) != skeleton(&new.sections, edited_path) {
        return Err(EditError::Restructure {
            message: format!(
                "the edit would change sections outside '{edited_path}' (for example by \
                 merging into an adjacent section); edit rejected, file unmodified"
            ),
        });
    }
    Ok(())
}

fn skeleton(sections: &[Section], excluded_subtree: &str) -> Vec<(String, u8)> {
    fn walk(sections: &[Section], excluded_subtree: &str, out: &mut Vec<(String, u8)>) {
        for section in sections {
            out.push((section.heading_path.clone(), section.level));
            if section.heading_path != excluded_subtree {
                walk(&section.children, excluded_subtree, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(sections, excluded_subtree, &mut out);
    out
}

fn write_atomically(path: &Path, content: &str) -> std::io::Result<()> {
    let mut temp = path.as_os_str().to_owned();
    temp.push(".context-tmp");
    let temp = PathBuf::from(temp);
    fs::write(&temp, content)?;
    // std::fs::rename maps to MoveFileExW + MOVEFILE_REPLACE_EXISTING on Windows, so the
    // destination is replaced atomically on the same volume.
    fs::rename(&temp, path).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })
}

fn hashed_outline(source: &str, sections: &[Section]) -> Vec<HashedOutlineSection> {
    sections
        .iter()
        .map(|section| HashedOutlineSection {
            heading: section
                .heading
                .clone()
                .unwrap_or_else(|| "Preamble".to_owned()),
            level: section.level,
            heading_path: section.heading_path.clone(),
            line_range: line_range(source, section.span),
            content_hash: section.content_hash.clone(),
            children: hashed_outline(source, &section.children),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_section_tree;

    const DOCUMENT: &str =
        "# Player\nlead\n\n## Skills\nskill intro\n\n### Gun\nbang\n\n## Inventory\nitems";

    fn section(source: &str, heading_path: &str) -> (Vec<Section>, Section) {
        let tree = build_section_tree(source, &parse(source));
        let found = find_section(&tree, heading_path)
            .unwrap_or_else(|| panic!("section {heading_path} exists"))
            .clone();
        (tree, found)
    }

    #[test]
    fn escape_validation_rejects_equal_and_higher_levels_and_names_the_heading() {
        for body in ["## Same Level", "# Higher\ntext", "Sneaky\n---"] {
            let error = validate_no_escape(body, 2, "Skills").expect_err("escapes level 2");
            let EditError::Escape { message } = &error else {
                panic!("expected escape error, got {error:?}");
            };
            assert!(
                message.contains("Skills"),
                "message names section: {message}"
            );
        }

        let error = validate_no_escape("## Same Level", 2, "Skills").expect_err("escape");
        assert!(error.message().contains("'Same Level'"));
        assert!(error.message().contains("level 2"));
    }

    #[test]
    fn escape_validation_accepts_deeper_and_container_nested_headings() {
        assert_eq!(validate_no_escape("### Deeper\ntext", 2, "Skills"), Ok(()));
        assert_eq!(validate_no_escape("> ## Quoted", 2, "Skills"), Ok(()));
        assert_eq!(validate_no_escape("plain text", 2, "Skills"), Ok(()));
    }

    #[test]
    fn escape_validation_sees_through_a_frontmatter_lookalike() {
        let body = "---\n# Sneaky\n---";
        let error = validate_no_escape(body, 2, "Skills").expect_err("hidden heading");
        assert!(error.message().contains("'Sneaky'"));
    }

    #[test]
    fn splice_replaces_only_the_body_and_preserves_separators() {
        let (_, gun) = section(DOCUMENT, "Player > Skills > Gun");
        let spliced = splice(DOCUMENT, gun.heading_span.unwrap(), gun.span, "boom");
        assert_eq!(
            spliced,
            "# Player\nlead\n\n## Skills\nskill intro\n\n### Gun\nboom\n\n## Inventory\nitems"
        );
    }

    #[test]
    fn splice_trims_trailing_whitespace_only_edges() {
        let (_, gun) = section(DOCUMENT, "Player > Skills > Gun");
        let spliced = splice(
            DOCUMENT,
            gun.heading_span.unwrap(),
            gun.span,
            "boom\n\n   \n",
        );
        assert!(spliced.contains("### Gun\nboom\n\n## Inventory"));
    }

    #[test]
    fn splice_keeps_interior_bytes_verbatim() {
        let (_, gun) = section(DOCUMENT, "Player > Skills > Gun");
        let body = "```\ncode  \n\n  more\n```";
        let spliced = splice(DOCUMENT, gun.heading_span.unwrap(), gun.span, body);
        assert!(spliced.contains(body));
    }

    #[test]
    fn splice_with_empty_body_leaves_only_the_heading_line() {
        let (_, gun) = section(DOCUMENT, "Player > Skills > Gun");
        let spliced = splice(DOCUMENT, gun.heading_span.unwrap(), gun.span, "  \n ");
        assert!(spliced.contains("### Gun\n\n## Inventory\nitems"));
    }

    #[test]
    fn splice_preserves_a_crlf_heading_terminator() {
        let source = "## Head\r\nold body\r\n\r\n## Next\r\nnext";
        let (_, head) = section(source, "Head");
        let spliced = splice(source, head.heading_span.unwrap(), head.span, "new");
        assert_eq!(spliced, "## Head\r\nnew\r\n\r\n## Next\r\nnext");
    }

    #[test]
    fn splice_edits_the_last_section_of_the_file() {
        let (_, inventory) = section(DOCUMENT, "Player > Inventory");
        let spliced = splice(
            DOCUMENT,
            inventory.heading_span.unwrap(),
            inventory.span,
            "sword",
        );
        assert!(spliced.ends_with("## Inventory\nsword"));
    }

    #[test]
    fn skeleton_verification_rejects_a_setext_glue_across_the_boundary() {
        // The old body ends in a code fence with a single-newline separator before a
        // setext heading; a new trailing paragraph glues into the underline's paragraph
        // and destroys the following section.
        let source = "## Head\n```\nfence\n```\nNext Title\n----------\nnext body";
        let old = EngineDocument::parse(source);
        let head = find_section(&old.sections, "Head")
            .expect("head exists")
            .clone();
        let spliced = splice(source, head.heading_span.unwrap(), head.span, "paragraph");
        let new = EngineDocument::parse(spliced);
        let error = verify_skeleton(&old, &new, "Head").expect_err("glue detected");
        assert!(matches!(error, EditError::Restructure { .. }));
    }

    #[test]
    fn skeleton_verification_allows_new_child_sections() {
        let source = "## Head\nbody\n\n## Next\nnext";
        let old = EngineDocument::parse(source);
        let head = find_section(&old.sections, "Head")
            .expect("head exists")
            .clone();
        let spliced = splice(
            source,
            head.heading_span.unwrap(),
            head.span,
            "intro\n\n### Child\nchild body",
        );
        let new = EngineDocument::parse(spliced);
        assert_eq!(verify_skeleton(&old, &new, "Head"), Ok(()));
    }
}
