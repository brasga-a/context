use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use context_parser::Span;
use serde::Serialize;

use crate::{EngineDocument, Section};

/// A half-open byte range suitable for serialized provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ByteRange {
    pub start: u32,
    pub end: u32,
}

/// A one-based inclusive source line range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

/// The body-free outline representation of one section.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OutlineSection {
    pub heading: String,
    pub level: u8,
    pub heading_path: String,
    pub line_range: LineRange,
    pub children: Vec<OutlineSection>,
}

/// Location details shared by retrieval and search results.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Provenance {
    pub file: String,
    pub heading_path: String,
    pub byte_range: ByteRange,
    pub line_range: LineRange,
    /// BLAKE3 hash of the section's exact source bytes — the `edit_section` guard token.
    pub content_hash: String,
}

/// One byte-exact section retrieval result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RetrievedSection {
    pub content: String,
    pub provenance: Provenance,
}

/// One ranked fuzzy heading match.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SearchResult {
    pub heading: String,
    pub score: u32,
    pub provenance: Provenance,
}

/// A non-fatal issue encountered while walking or reading a vault.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VaultDiagnostic {
    pub file: Option<String>,
    pub message: String,
}

/// A fatal vault construction error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultError {
    message: String,
}

impl VaultError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for VaultError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for VaultError {}

/// An exact lookup error with deterministic nearest-match suggestions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RetrievalError {
    pub message: String,
    pub suggestions: Vec<String>,
}

impl fmt::Display for RetrievalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)?;
        if !self.suggestions.is_empty() {
            write!(
                formatter,
                "; nearest matches: {}",
                self.suggestions.join(", ")
            )?;
        }
        Ok(())
    }
}

impl Error for RetrievalError {}

/// An in-memory structural index over all Markdown files below one vault root.
#[derive(Clone, Debug)]
pub struct VaultIndex {
    pub(crate) root: PathBuf,
    pub(crate) documents: BTreeMap<String, EngineDocument>,
    /// Non-fatal indexing and document diagnostics.
    pub diagnostics: Vec<VaultDiagnostic>,
}

impl VaultIndex {
    /// Walks and indexes a vault directory recursively.
    pub fn build(root: impl AsRef<Path>) -> Result<Self, VaultError> {
        let root = root.as_ref().to_path_buf();
        if !root.exists() {
            return Err(VaultError::new(format!(
                "vault path '{}' does not exist",
                root.display()
            )));
        }
        if !root.is_dir() {
            return Err(VaultError::new(format!(
                "vault path '{}' is not a directory",
                root.display()
            )));
        }

        let mut documents = BTreeMap::new();
        let mut diagnostics = Vec::new();
        let mut directories = vec![root.clone()];

        while let Some(directory) = directories.pop() {
            let entries = match fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) => {
                    diagnostics.push(VaultDiagnostic {
                        file: relative_path(&root, &directory),
                        message: format!("failed to read directory: {error}"),
                    });
                    continue;
                }
            };

            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => {
                        diagnostics.push(VaultDiagnostic {
                            file: relative_path(&root, &directory),
                            message: format!("failed to read directory entry: {error}"),
                        });
                        continue;
                    }
                };
                let path = entry.path();
                let file_type = match entry.file_type() {
                    Ok(file_type) => file_type,
                    Err(error) => {
                        diagnostics.push(VaultDiagnostic {
                            file: relative_path(&root, &path),
                            message: format!("failed to inspect path: {error}"),
                        });
                        continue;
                    }
                };

                if file_type.is_dir() {
                    directories.push(path);
                    continue;
                }
                if !file_type.is_file() || !is_markdown(&path) {
                    continue;
                }

                let Some(file) = relative_path(&root, &path) else {
                    diagnostics.push(VaultDiagnostic {
                        file: Some(path.to_string_lossy().into_owned()),
                        message: "vault-relative path is not valid Unicode".to_owned(),
                    });
                    continue;
                };
                let source = match fs::read_to_string(&path) {
                    Ok(source) => source,
                    Err(error) => {
                        diagnostics.push(VaultDiagnostic {
                            file: Some(file),
                            message: format!("failed to read Markdown file: {error}"),
                        });
                        continue;
                    }
                };
                let document = EngineDocument::parse(source);
                diagnostics.extend(
                    document
                        .diagnostics
                        .iter()
                        .map(|diagnostic| VaultDiagnostic {
                            file: Some(file.clone()),
                            message: diagnostic.message.clone(),
                        }),
                );
                documents.insert(file, document);
            }
        }

        diagnostics.sort_by(|left, right| {
            left.file
                .cmp(&right.file)
                .then_with(|| left.message.cmp(&right.message))
        });
        Ok(Self {
            root,
            documents,
            diagnostics,
        })
    }

    /// Returns the configured vault root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns indexed vault-relative paths in deterministic order.
    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.documents.keys().map(String::as_str)
    }

    /// Returns the body-free outline for one indexed file.
    pub fn outline(&self, file: &str) -> Result<Vec<OutlineSection>, RetrievalError> {
        let (_, document) = self.document(file)?;
        Ok(document
            .sections
            .iter()
            .map(|section| outline_section(document.source(), section))
            .collect())
    }

    /// Retrieves one section by file and exact heading path.
    pub fn get_section(
        &self,
        file: &str,
        heading_path: &str,
    ) -> Result<RetrievedSection, RetrievalError> {
        let (file, document) = self.document(file)?;
        let Some(section) = find_section(&document.sections, heading_path) else {
            let mut paths = Vec::new();
            visit_sections(&document.sections, &mut |section| {
                paths.push(section.heading_path.clone());
            });
            return Err(RetrievalError {
                message: format!("heading path '{heading_path}' was not found in file '{file}'"),
                suggestions: nearest(heading_path, paths.iter().map(String::as_str), 3),
            });
        };

        Ok(RetrievedSection {
            content: section.span.slice(document.source()).to_owned(),
            provenance: provenance(file, document.source(), section),
        })
    }

    /// Searches heading text and paths using deterministic normalized-token ranking.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let query_tokens = normalized_tokens(query);
        if query_tokens.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        for (file, document) in &self.documents {
            visit_sections(&document.sections, &mut |section| {
                let Some(heading) = section.heading.as_ref() else {
                    return;
                };
                let heading_tokens = normalized_tokens(heading);
                let path_tokens = normalized_tokens(&section.heading_path);
                let score = match_score(&query_tokens, &heading_tokens, &path_tokens);
                if score > 0 {
                    results.push(SearchResult {
                        heading: heading.clone(),
                        score,
                        provenance: provenance(file, document.source(), section),
                    });
                }
            });
        }

        results.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| {
                    path_depth(&left.provenance.heading_path)
                        .cmp(&path_depth(&right.provenance.heading_path))
                })
                .then_with(|| left.provenance.file.cmp(&right.provenance.file))
                .then_with(|| {
                    left.provenance
                        .heading_path
                        .cmp(&right.provenance.heading_path)
                })
        });
        results
    }

    fn document(&self, file: &str) -> Result<(&str, &EngineDocument), RetrievalError> {
        let normalized = normalize_request_path(file).unwrap_or_else(|| file.replace('\\', "/"));
        self.documents
            .get_key_value(&normalized)
            .map(|(file, document)| (file.as_str(), document))
            .ok_or_else(|| RetrievalError {
                message: format!("file '{file}' is not indexed"),
                suggestions: nearest(file, self.documents.keys().map(String::as_str), 3),
            })
    }
}

fn outline_section(source: &str, section: &Section) -> OutlineSection {
    OutlineSection {
        heading: section
            .heading
            .clone()
            .unwrap_or_else(|| "Preamble".to_owned()),
        level: section.level,
        heading_path: section.heading_path.clone(),
        line_range: line_range(source, section.span),
        children: section
            .children
            .iter()
            .map(|child| outline_section(source, child))
            .collect(),
    }
}

fn provenance(file: &str, source: &str, section: &Section) -> Provenance {
    Provenance {
        file: file.to_owned(),
        heading_path: section.heading_path.clone(),
        byte_range: ByteRange {
            start: section.span.start,
            end: section.span.end,
        },
        line_range: line_range(source, section.span),
        content_hash: section.content_hash.clone(),
    }
}

pub(crate) fn line_range(source: &str, span: Span) -> LineRange {
    let start = line_number(source, span.start);
    let end_offset = span.end.saturating_sub(1).max(span.start);
    LineRange {
        start,
        end: line_number(source, end_offset),
    }
}

fn line_number(source: &str, offset: u32) -> u32 {
    source.as_bytes()[..offset as usize]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count() as u32
        + 1
}

pub(crate) fn find_section<'a>(sections: &'a [Section], heading_path: &str) -> Option<&'a Section> {
    for section in sections {
        if section.heading_path == heading_path {
            return Some(section);
        }
        if let Some(found) = find_section(&section.children, heading_path) {
            return Some(found);
        }
    }
    None
}

pub(crate) fn visit_sections(sections: &[Section], visitor: &mut impl FnMut(&Section)) {
    for section in sections {
        visitor(section);
        visit_sections(&section.children, visitor);
    }
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut components = Vec::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            continue;
        };
        components.push(component.to_str()?);
    }
    Some(components.join("/"))
}

pub(crate) fn normalize_request_path(path: &str) -> Option<String> {
    let replaced = path.replace('\\', "/");
    let mut components = Vec::new();
    for component in replaced.split('/') {
        match component {
            "" | "." => {}
            ".." => return None,
            component => components.push(component),
        }
    }
    Some(components.join("/"))
}

fn normalized_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    for character in value.chars() {
        if character.is_alphanumeric() {
            token.extend(character.to_lowercase());
        } else if !token.is_empty() {
            tokens.push(std::mem::take(&mut token));
        }
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    tokens
}

fn match_score(query: &[String], heading: &[String], path: &[String]) -> u32 {
    if query == path {
        return 40_000;
    }
    if query == heading {
        return 30_000;
    }

    let mut sorted_query = query.to_vec();
    sorted_query.sort();
    let mut sorted_heading = heading.to_vec();
    sorted_heading.sort();
    if sorted_query == sorted_heading {
        return 29_000;
    }

    let path_matches = token_matches(query, path);
    let heading_matches = token_matches(query, heading);
    let best_matches = path_matches.max(heading_matches);
    if best_matches == 0 {
        return 0;
    }

    let all_query_tokens_match = best_matches == query.len();
    let exact_tokens = query
        .iter()
        .filter(|query_token| path.iter().any(|path_token| path_token == *query_token))
        .count();
    if all_query_tokens_match {
        20_000 + exact_tokens as u32 * 100
    } else {
        10_000 + best_matches as u32 * 100 + exact_tokens as u32
    }
}

fn token_matches(query: &[String], candidate: &[String]) -> usize {
    query
        .iter()
        .filter(|query_token| {
            candidate.iter().any(|candidate_token| {
                candidate_token.starts_with(query_token.as_str())
                    || query_token.starts_with(candidate_token.as_str())
            })
        })
        .count()
}

fn path_depth(path: &str) -> usize {
    path.matches(" > ").count()
}

pub(crate) fn nearest<'a>(
    needle: &str,
    candidates: impl Iterator<Item = &'a str>,
    limit: usize,
) -> Vec<String> {
    let needle = needle.to_lowercase();
    let mut candidates: Vec<_> = candidates
        .map(|candidate| {
            (
                levenshtein(&needle, &candidate.to_lowercase()),
                candidate.to_owned(),
            )
        })
        .collect();
    candidates.sort();
    candidates
        .into_iter()
        .take(limit)
        .map(|(_, candidate)| candidate)
        .collect()
}

fn levenshtein(left: &str, right: &str) -> usize {
    let mut previous: Vec<usize> = (0..=right.chars().count()).collect();
    let mut current = vec![0; previous.len()];

    for (left_index, left_character) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_character) in right.chars().enumerate() {
            current[right_index + 1] = if left_character == right_character {
                previous[right_index]
            } else {
                1 + previous[right_index]
                    .min(current[right_index])
                    .min(previous[right_index + 1])
            };
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right.chars().count()]
}
