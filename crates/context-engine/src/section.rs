use std::collections::HashMap;

use context_parser::{Block, Document, Inline, Span};

/// A source-backed Markdown section derived from a heading and its following blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    /// Plain-text heading content, or `None` for the synthetic preamble.
    pub heading: Option<String>,
    /// Markdown heading level, or zero for the synthetic preamble.
    pub level: u8,
    /// Deterministically disambiguated breadcrumb identity within the document.
    pub heading_path: String,
    /// Exact half-open byte range covered by this section.
    pub span: Span,
    /// Lower-level heading sections nested below this section.
    pub children: Vec<Section>,
    /// BLAKE3 hash of the exact source bytes covered by `span`.
    pub content_hash: String,
}

/// Derives a nested section tree from the parser's flat top-level block stream.
pub fn build_section_tree(source: &str, document: &Document) -> Vec<Section> {
    let first_heading = document
        .children
        .iter()
        .position(|block| matches!(block, Block::Heading { .. }));
    let mut sections = Vec::new();
    let mut sibling_names = HashMap::new();

    let heading_start = match first_heading {
        Some(index) => index,
        None if document.children.is_empty() => return sections,
        None => document.children.len(),
    };

    if heading_start > 0 {
        let preamble_span = Span {
            start: block_span(&document.children[0])
                .expect("the parser emits spans for every known block")
                .start,
            end: block_span(&document.children[heading_start - 1])
                .expect("the parser emits spans for every known block")
                .end,
        };
        sibling_names.insert("Preamble".to_owned(), 1);
        sections.push(Section {
            heading: None,
            level: 0,
            heading_path: "Preamble".to_owned(),
            span: preamble_span,
            children: Vec::new(),
            content_hash: hash_span(source, preamble_span),
        });
    }

    if let Some(first_heading) = first_heading {
        let (mut heading_sections, _) = parse_siblings(
            source,
            &document.children,
            first_heading,
            0,
            "",
            sibling_names,
        );
        sections.append(&mut heading_sections);
    }

    sections
}

fn parse_siblings(
    source: &str,
    blocks: &[Block],
    mut index: usize,
    parent_level: u8,
    parent_path: &str,
    mut sibling_names: HashMap<String, usize>,
) -> (Vec<Section>, usize) {
    let mut sections = Vec::new();

    while index < blocks.len() {
        let Block::Heading {
            span,
            level,
            content,
            ..
        } = &blocks[index]
        else {
            index += 1;
            continue;
        };

        if *level <= parent_level {
            break;
        }

        let heading = inline_text(source, content).trim().to_owned();
        let occurrence = sibling_names
            .entry(heading.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        let identity = if *occurrence == 1 {
            heading.clone()
        } else {
            format!("{heading}[{occurrence}]")
        };
        let heading_path = if parent_path.is_empty() {
            identity
        } else {
            format!("{parent_path} > {identity}")
        };

        let mut end = span.end;
        index += 1;
        let mut children = Vec::new();

        while index < blocks.len() {
            match &blocks[index] {
                Block::Heading {
                    level: next_level, ..
                } if *next_level <= *level => break,
                Block::Heading { .. } => {
                    let (nested, next_index) = parse_siblings(
                        source,
                        blocks,
                        index,
                        *level,
                        &heading_path,
                        HashMap::new(),
                    );
                    if let Some(last) = nested.last() {
                        end = end.max(last.span.end);
                    }
                    children.extend(nested);
                    index = next_index;
                }
                block => {
                    if let Some(block_span) = block_span(block) {
                        end = end.max(block_span.end);
                    }
                    index += 1;
                }
            }
        }

        let section_span = Span {
            start: span.start,
            end,
        };
        sections.push(Section {
            heading: Some(heading),
            level: *level,
            heading_path,
            span: section_span,
            children,
            content_hash: hash_span(source, section_span),
        });
    }

    (sections, index)
}

fn block_span(block: &Block) -> Option<Span> {
    match block {
        Block::Paragraph { span, .. }
        | Block::Heading { span, .. }
        | Block::ThematicBreak { span }
        | Block::CodeBlock { span, .. }
        | Block::BlockQuote { span, .. }
        | Block::List { span, .. }
        | Block::Table { span, .. }
        | Block::FootnoteDefinition { span, .. } => Some(*span),
        _ => None,
    }
}

fn inline_text(source: &str, inlines: &[Inline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text { span } => text.push_str(span.slice(source)),
            Inline::Escaped { span } => {
                text.push_str(span.slice(source).strip_prefix('\\').unwrap_or_default());
            }
            Inline::CharacterReference { value, .. } => text.push_str(value),
            Inline::CodeSpan { literal, .. } | Inline::Math { literal, .. } => {
                for span in literal {
                    text.push_str(span.slice(source));
                }
            }
            Inline::Emphasis { children, .. }
            | Inline::Strong { children, .. }
            | Inline::Strikethrough { children, .. }
            | Inline::Highlight { children, .. }
            | Inline::Link { children, .. }
            | Inline::Image { children, .. } => text.push_str(&inline_text(source, children)),
            Inline::Autolink { uri, .. } => text.push_str(uri.slice(source)),
            Inline::WikiLink { target, label, .. } => {
                text.push_str(label.unwrap_or(*target).slice(source));
            }
            Inline::FootnoteReference { label, .. } => text.push_str(label.slice(source)),
            Inline::SoftBreak { .. } | Inline::HardBreak { .. } => text.push(' '),
            _ => {}
        }
    }
    text
}

fn hash_span(source: &str, span: Span) -> String {
    blake3::hash(span.slice(source).as_bytes())
        .to_hex()
        .to_string()
}

#[cfg(test)]
mod tests {
    use context_parser::parse;

    use super::*;

    const NESTED_DOCUMENT: &str = "intro

# Player
lead

## Skills
skill intro

### Gun
bang

### Gun
bang2

## Inventory
items";

    #[test]
    fn derives_nested_sections_preamble_paths_and_exact_spans() {
        let tree = build_section_tree(NESTED_DOCUMENT, &parse(NESTED_DOCUMENT));

        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].heading, None);
        assert_eq!(tree[0].level, 0);
        assert_eq!(tree[0].heading_path, "Preamble");
        assert_eq!(tree[0].span.slice(NESTED_DOCUMENT), "intro");

        let player = &tree[1];
        assert_eq!(player.heading.as_deref(), Some("Player"));
        assert_eq!(player.heading_path, "Player");
        assert_eq!(player.children.len(), 2);
        assert_eq!(player.children[0].heading_path, "Player > Skills");
        assert_eq!(player.children[1].heading_path, "Player > Inventory");

        let skills = &player.children[0];
        assert_eq!(skills.children.len(), 2);
        assert_eq!(skills.children[0].heading_path, "Player > Skills > Gun");
        assert_eq!(skills.children[1].heading_path, "Player > Skills > Gun[2]");
        assert_eq!(
            skills.span.slice(NESTED_DOCUMENT),
            "## Skills\nskill intro\n\n### Gun\nbang\n\n### Gun\nbang2"
        );
        assert!(skills.children[0].span.start >= skills.span.start);
        assert!(skills.children[1].span.end <= skills.span.end);
        assert_eq!(
            player.span.slice(NESTED_DOCUMENT),
            &NESTED_DOCUMENT[NESTED_DOCUMENT.find("# Player").unwrap()..]
        );
    }

    #[test]
    fn nests_skipped_levels_directly_without_synthetic_sections() {
        let source = "# Root\n\n### Deep\nbody";
        let tree = build_section_tree(source, &parse(source));

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].level, 3);
        assert_eq!(tree[0].children[0].heading_path, "Root > Deep");
    }

    #[test]
    fn hashes_are_stable_and_sensitive_to_exact_section_bytes() {
        let unchanged = build_section_tree(NESTED_DOCUMENT, &parse(NESTED_DOCUMENT));
        let reparsed = build_section_tree(NESTED_DOCUMENT, &parse(NESTED_DOCUMENT));
        assert_eq!(unchanged, reparsed);

        let edited_source = NESTED_DOCUMENT.replace("bang\n", "boom\n");
        let edited = build_section_tree(&edited_source, &parse(&edited_source));
        let original_player = &unchanged[1];
        let edited_player = &edited[1];

        assert_ne!(
            original_player.children[0].children[0].content_hash,
            edited_player.children[0].children[0].content_hash
        );
        assert_eq!(
            original_player.children[1].content_hash,
            edited_player.children[1].content_hash
        );
    }

    #[test]
    fn a_heading_free_document_is_one_preamble_section() {
        let source = "first\n\nsecond";
        let tree = build_section_tree(source, &parse(source));

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].heading_path, "Preamble");
        assert_eq!(tree[0].span.slice(source), source);
    }
}
