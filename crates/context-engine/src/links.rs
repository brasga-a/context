use std::collections::BTreeMap;

use context_parser::{Block, Inline, Span, parse};
use serde::Serialize;

use crate::EngineDocument;
use crate::vault::{Provenance, VaultDiagnostic, provenance, section_at, visit_sections};

/// One resolved wikilink pointing at a section (or whole file).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Backlink {
    /// Provenance of the section containing the link.
    pub from: Provenance,
    /// The link's target text exactly as written, before `#` splitting.
    pub raw_target: String,
    /// The resolved target's heading path, or `None` for a whole-file link.
    pub target_heading_path: Option<String>,
}

/// Resolves every wikilink in `documents` into backlinks (keyed by resolved target file) and
/// diagnostics for anything that did not resolve. A full, vault-wide pass: a change to one
/// file's headings can flip whether another file's link resolves, so this cannot be computed
/// incrementally per file.
pub(crate) fn resolve_links(
    documents: &BTreeMap<String, EngineDocument>,
) -> (BTreeMap<String, Vec<Backlink>>, Vec<VaultDiagnostic>) {
    let mut backlinks: BTreeMap<String, Vec<Backlink>> = BTreeMap::new();
    let mut diagnostics = Vec::new();

    for (file, document) in documents {
        let parsed = parse(document.source());
        for (link_span, target_span) in collect_wikilinks(&parsed.children) {
            let Some(owning) = section_at(&document.sections, link_span.start) else {
                continue;
            };
            let raw_target = target_span.slice(document.source());
            let (file_part, heading_part) = split_target(raw_target);

            let target_file = match resolve_file(file, file_part, documents) {
                Ok(target_file) => target_file,
                Err(reason) => {
                    diagnostics.push(unresolved(
                        file,
                        owning.heading_path.as_str(),
                        raw_target,
                        &reason,
                    ));
                    continue;
                }
            };

            let target_heading_path = match heading_part {
                None => None,
                Some(heading_text) => {
                    let target_document = &documents[&target_file];
                    match resolve_heading(target_document, heading_text) {
                        Ok(path) => Some(path),
                        Err(reason) => {
                            diagnostics.push(unresolved(
                                file,
                                owning.heading_path.as_str(),
                                raw_target,
                                &reason,
                            ));
                            continue;
                        }
                    }
                }
            };

            backlinks.entry(target_file).or_default().push(Backlink {
                from: provenance(file, document.source(), owning),
                raw_target: raw_target.to_owned(),
                target_heading_path,
            });
        }
    }

    for entries in backlinks.values_mut() {
        entries.sort_by(|left, right| {
            left.from
                .file
                .cmp(&right.from.file)
                .then_with(|| left.from.heading_path.cmp(&right.from.heading_path))
        });
    }
    diagnostics.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then_with(|| left.message.cmp(&right.message))
    });
    (backlinks, diagnostics)
}

/// Splits a wikilink target on its first `#`: file part, then heading part if present.
fn split_target(text: &str) -> (&str, Option<&str>) {
    match text.find('#') {
        Some(index) => (&text[..index], Some(&text[index + 1..])),
        None => (text, None),
    }
}

fn file_stem(path: &str) -> &str {
    let last = path.rsplit('/').next().unwrap_or(path);
    last.strip_suffix(".md").unwrap_or(last)
}

/// Resolves a wikilink's file part to exactly one indexed file, vault-wide.
fn resolve_file(
    current_file: &str,
    file_part: &str,
    documents: &BTreeMap<String, EngineDocument>,
) -> Result<String, String> {
    if file_part.is_empty() {
        return Ok(current_file.to_owned());
    }
    if file_part.contains('/') {
        let normalized = file_part.strip_suffix(".md").unwrap_or(file_part);
        let with_md = format!("{normalized}.md");
        if documents.contains_key(&with_md) {
            return Ok(with_md);
        }
        if documents.contains_key(file_part) {
            return Ok(file_part.to_owned());
        }
        return Err(format!("no file matches path '{file_part}'"));
    }

    let mut matches = documents.keys().filter(|key| file_stem(key) == file_part);
    let Some(first) = matches.next() else {
        return Err(format!("no file matches '{file_part}'"));
    };
    if matches.next().is_some() {
        let candidates: Vec<_> = documents
            .keys()
            .filter(|key| file_stem(key) == file_part)
            .cloned()
            .collect();
        return Err(format!(
            "'{file_part}' matches multiple files: {}",
            candidates.join(", ")
        ));
    }
    Ok(first.clone())
}

/// Resolves a wikilink's heading part to exactly one section's heading path within a file.
fn resolve_heading(document: &EngineDocument, heading_text: &str) -> Result<String, String> {
    let mut matches = Vec::new();
    visit_sections(&document.sections, &mut |section| {
        if section.heading.as_deref() == Some(heading_text) {
            matches.push(section.heading_path.clone());
        }
    });
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(format!("no heading named '{heading_text}'")),
        _ => Err(format!(
            "heading '{heading_text}' is ambiguous: {}",
            matches.join(", ")
        )),
    }
}

fn unresolved(file: &str, owning_path: &str, raw_target: &str, reason: &str) -> VaultDiagnostic {
    VaultDiagnostic {
        file: Some(file.to_owned()),
        message: format!(
            "wikilink '[[{raw_target}]]' in section '{owning_path}' of '{file}' does not \
             resolve: {reason}"
        ),
    }
}

/// Collects every wikilink's (link span, target span) across a document's full block tree:
/// paragraphs, headings, list items, table cells, blockquotes, and footnote definitions.
fn collect_wikilinks(blocks: &[Block]) -> Vec<(Span, Span)> {
    let mut found = Vec::new();
    walk_blocks(blocks, &mut found);
    found
}

fn walk_blocks(blocks: &[Block], found: &mut Vec<(Span, Span)>) {
    for block in blocks {
        match block {
            Block::Paragraph { content, .. } | Block::Heading { content, .. } => {
                walk_inlines(content, found);
            }
            Block::BlockQuote { children, .. } | Block::FootnoteDefinition { children, .. } => {
                walk_blocks(children, found);
            }
            Block::List { items, .. } => {
                for item in items {
                    walk_blocks(&item.children, found);
                }
            }
            Block::Table { head, rows, .. } => {
                for cell in &head.cells {
                    walk_inlines(&cell.content, found);
                }
                for row in rows {
                    for cell in &row.cells {
                        walk_inlines(&cell.content, found);
                    }
                }
            }
            Block::ThematicBreak { .. } | Block::CodeBlock { .. } => {}
            _ => {}
        }
    }
}

fn walk_inlines(inlines: &[Inline], found: &mut Vec<(Span, Span)>) {
    for inline in inlines {
        match inline {
            Inline::WikiLink { span, target, .. } => found.push((*span, *target)),
            Inline::Emphasis { children, .. }
            | Inline::Strong { children, .. }
            | Inline::Strikethrough { children, .. }
            | Inline::Highlight { children, .. }
            | Inline::Link { children, .. }
            | Inline::Image { children, .. } => walk_inlines(children, found),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_parser::parse;

    fn document(source: &str) -> EngineDocument {
        EngineDocument::parse(source)
    }

    #[test]
    fn split_target_separates_file_and_heading_parts() {
        assert_eq!(split_target("Weapons"), ("Weapons", None));
        assert_eq!(split_target("Weapons#Gun"), ("Weapons", Some("Gun")));
        assert_eq!(split_target("#Gun"), ("", Some("Gun")));
        assert_eq!(split_target("A#B#C"), ("A", Some("B#C")));
    }

    #[test]
    fn collect_wikilinks_finds_links_in_every_container() {
        let source = "\
# Head
para [[Para]]

> quote [[Quote]]

- item [[Item]]

| h | h2 |
|---|----|
| [[Cell]] | x |

[^note]: foot [[Foot]]
";
        let parsed = parse(source);
        let found = collect_wikilinks(&parsed.children);
        let mut targets: Vec<_> = found
            .iter()
            .map(|(_, target)| target.slice(source))
            .collect();
        targets.sort();
        assert_eq!(targets, ["Cell", "Foot", "Item", "Para", "Quote"]);
    }

    #[test]
    fn collect_wikilinks_finds_links_nested_inside_emphasis() {
        let source = "para **strong [[Nested]] text**";
        let parsed = parse(source);
        let found = collect_wikilinks(&parsed.children);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1.slice(source), "Nested");
    }

    #[test]
    fn section_at_finds_the_preamble_and_the_deepest_nested_section() {
        use crate::vault::section_at;
        let source = "intro text\n\n# Player\n\n## Skills\n\n### Gun\nbody";
        let doc = document(source);

        let preamble_offset = source.find("intro").unwrap() as u32;
        let preamble = section_at(&doc.sections, preamble_offset).expect("preamble found");
        assert_eq!(preamble.heading_path, "Preamble");

        let gun_offset = source.find("body").unwrap() as u32;
        let deepest = section_at(&doc.sections, gun_offset).expect("deepest section found");
        assert_eq!(deepest.heading_path, "Player > Skills > Gun");

        assert!(section_at(&doc.sections, source.len() as u32).is_none());
    }

    #[test]
    fn resolve_file_matches_stem_across_directories() {
        let mut documents = BTreeMap::new();
        documents.insert("lore/weapons.md".to_owned(), document("# Weapons"));
        documents.insert("player.md".to_owned(), document("# Player"));

        assert_eq!(
            resolve_file("player.md", "weapons", &documents),
            Ok("lore/weapons.md".to_owned())
        );
    }

    #[test]
    fn resolve_file_self_link_uses_the_current_file() {
        let documents = BTreeMap::new();
        assert_eq!(
            resolve_file("player.md", "", &documents),
            Ok("player.md".to_owned())
        );
    }

    #[test]
    fn resolve_file_no_match_and_ambiguous_match_are_errors() {
        let mut documents = BTreeMap::new();
        documents.insert("player.md".to_owned(), document("# Player"));
        assert!(resolve_file("player.md", "Nowhere", &documents).is_err());

        let mut ambiguous = BTreeMap::new();
        ambiguous.insert("a/weapons.md".to_owned(), document("# A"));
        ambiguous.insert("b/weapons.md".to_owned(), document("# B"));
        let error = resolve_file("player.md", "weapons", &ambiguous).expect_err("ambiguous");
        assert!(error.contains("a/weapons.md"));
        assert!(error.contains("b/weapons.md"));
    }

    #[test]
    fn resolve_heading_matches_by_text_not_full_path() {
        let doc = document("# Player\n\n## Skills\n\n### Gun\nbody");
        assert_eq!(
            resolve_heading(&doc, "Gun"),
            Ok("Player > Skills > Gun".to_owned())
        );
        assert!(resolve_heading(&doc, "Nothing").is_err());
    }

    #[test]
    fn resolve_heading_ambiguous_lists_both_candidates() {
        let doc = document("## Gun Skill\nA\n\n## Gun Skill\nB");
        let error = resolve_heading(&doc, "Gun Skill").expect_err("ambiguous");
        assert!(error.contains("Gun Skill"));
        assert!(error.contains("Gun Skill[2]"));
    }

    #[test]
    fn resolve_links_produces_backlinks_and_diagnostics_end_to_end() {
        let mut documents = BTreeMap::new();
        documents.insert(
            "weapons.md".to_owned(),
            document("## Gun Skill\nAdvanced handling."),
        );
        documents.insert(
            "player.md".to_owned(),
            document(
                "## Notes\nSee [[weapons]] and [[weapons#Gun Skill]]. Self: [[#Notes]]. \
                 Broken: [[Nowhere]] and [[weapons#Missing]].",
            ),
        );

        let (backlinks, diagnostics) = resolve_links(&documents);

        let weapons_backlinks = &backlinks["weapons.md"];
        assert_eq!(weapons_backlinks.len(), 2);
        assert!(
            weapons_backlinks
                .iter()
                .any(|link| link.raw_target == "weapons" && link.target_heading_path.is_none())
        );
        assert!(
            weapons_backlinks
                .iter()
                .any(|link| link.raw_target == "weapons#Gun Skill"
                    && link.target_heading_path.as_deref() == Some("Gun Skill"))
        );

        let player_backlinks = &backlinks["player.md"];
        assert_eq!(player_backlinks.len(), 1);
        assert_eq!(
            player_backlinks[0].target_heading_path.as_deref(),
            Some("Notes")
        );

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().any(|d| d.message.contains("Nowhere")));
        assert!(diagnostics.iter().any(|d| d.message.contains("Missing")));
    }
}
