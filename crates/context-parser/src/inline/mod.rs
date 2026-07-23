use std::collections::{HashMap, HashSet};

use memchr::{memchr, memchr2};
use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

use crate::{
    Block, Document, HeadingKind, Inline, LinkDefinition, LinkTarget, ListItem, Span, TableCell,
    TableRow,
    ast::{RawBlock, RawDocument, RawListItem, RawTableRow},
};

mod entities;

pub(crate) fn finish_document(source: &str, raw: RawDocument) -> Document {
    let mut definitions = Vec::new();
    let mut footnotes = HashSet::new();
    let raw_children = extract_definitions(source, raw.children, &mut definitions, &mut footnotes);
    let mut references = HashMap::new();
    for (index, definition) in definitions.iter().enumerate() {
        references
            .entry(normalize_label(definition.label.slice(source)))
            .or_insert(index);
    }
    Document {
        frontmatter: raw.frontmatter,
        children: finish_blocks(source, raw_children, &definitions, &references, &footnotes),
        definitions,
        diagnostics: raw.diagnostics,
    }
}

fn finish_blocks(
    source: &str,
    blocks: Vec<RawBlock>,
    definitions: &[LinkDefinition],
    references: &HashMap<String, usize>,
    footnotes: &HashSet<String>,
) -> Vec<Block> {
    blocks
        .into_iter()
        .map(|block| finish_block(source, block, definitions, references, footnotes))
        .collect()
}

fn finish_block(
    source: &str,
    block: RawBlock,
    definitions: &[LinkDefinition],
    references: &HashMap<String, usize>,
    footnotes: &HashSet<String>,
) -> Block {
    match block {
        RawBlock::Paragraph { span, fragments } => Block::Paragraph {
            span,
            content: parse_fragments(source, &fragments, definitions, references, footnotes),
        },
        RawBlock::Heading {
            span,
            level,
            kind,
            fragments,
        } => Block::Heading {
            span,
            level,
            kind,
            content: parse_fragments(source, &fragments, definitions, references, footnotes),
        },
        RawBlock::ThematicBreak { span } => Block::ThematicBreak { span },
        RawBlock::CodeBlock {
            span,
            kind,
            literal,
        } => Block::CodeBlock {
            span,
            kind,
            literal,
        },
        RawBlock::BlockQuote { span, children } => Block::BlockQuote {
            span,
            children: finish_blocks(source, children, definitions, references, footnotes),
        },
        RawBlock::List {
            span,
            kind,
            tight,
            items,
        } => Block::List {
            span,
            kind,
            tight,
            items: items
                .into_iter()
                .map(|item| ListItem {
                    span: item.span,
                    task: item.task,
                    children: finish_blocks(
                        source,
                        item.children,
                        definitions,
                        references,
                        footnotes,
                    ),
                })
                .collect(),
        },
        RawBlock::Table {
            span,
            alignments,
            head,
            rows,
        } => Block::Table {
            span,
            alignments,
            head: finish_table_row(source, head, definitions, references, footnotes),
            rows: rows
                .into_iter()
                .map(|row| finish_table_row(source, row, definitions, references, footnotes))
                .collect(),
        },
        RawBlock::FootnoteDefinition {
            span,
            label,
            children,
        } => Block::FootnoteDefinition {
            span,
            label,
            children: finish_blocks(source, children, definitions, references, footnotes),
        },
    }
}

fn finish_table_row(
    source: &str,
    row: RawTableRow,
    definitions: &[LinkDefinition],
    references: &HashMap<String, usize>,
    footnotes: &HashSet<String>,
) -> TableRow {
    TableRow {
        span: row.span,
        cells: row
            .cells
            .into_iter()
            .map(|span| TableCell {
                span,
                content: parse_fragments(source, &[span], definitions, references, footnotes),
            })
            .collect(),
    }
}

fn extract_definitions(
    source: &str,
    blocks: Vec<RawBlock>,
    definitions: &mut Vec<LinkDefinition>,
    footnotes: &mut HashSet<String>,
) -> Vec<RawBlock> {
    let mut retained = Vec::with_capacity(blocks.len());
    let mut blocks = blocks.into_iter().peekable();
    while let Some(block) = blocks.next() {
        match block {
            RawBlock::Paragraph {
                span,
                mut fragments,
            } => {
                if let Some((label, definition)) =
                    parse_footnote_definition(source, span, &mut fragments)
                {
                    footnotes.insert(normalize_label(label.slice(source)));
                    retained.push(definition);
                    continue;
                }
                let definition_count =
                    extract_leading_link_definitions(source, &fragments, definitions);
                fragments.drain(..definition_count);
                if let Some(first) = fragments.first() {
                    retained.push(RawBlock::Paragraph {
                        span: Span {
                            start: first.start,
                            end: span.end,
                        },
                        fragments,
                    });
                }
            }
            RawBlock::Heading {
                span,
                level,
                kind: HeadingKind::Setext,
                mut fragments,
            } => {
                let definition_count =
                    extract_leading_link_definitions(source, &fragments, definitions);
                fragments.drain(..definition_count);
                if let Some(first) = fragments.first() {
                    retained.push(RawBlock::Heading {
                        span: Span {
                            start: first.start,
                            end: span.end,
                        },
                        level,
                        kind: HeadingKind::Setext,
                        fragments,
                    });
                } else {
                    let underline = setext_underline_span(source, span, level);
                    if level == 1 {
                        let mut paragraph_span = underline;
                        let mut paragraph_fragments = vec![underline];
                        let merge_next = matches!(
                            blocks.peek(),
                            Some(RawBlock::Paragraph { fragments, .. })
                                if fragments.first().is_some_and(|first| {
                                    starts_on_next_line(source, underline.end, first.start)
                                })
                        );
                        if merge_next {
                            let Some(RawBlock::Paragraph {
                                span,
                                fragments: mut following_fragments,
                            }) = blocks.next()
                            else {
                                unreachable!("peeked paragraph must remain the next block");
                            };
                            paragraph_span.end = span.end;
                            paragraph_fragments.append(&mut following_fragments);
                        }
                        retained.push(RawBlock::Paragraph {
                            span: paragraph_span,
                            fragments: paragraph_fragments,
                        });
                    } else {
                        retained.push(RawBlock::ThematicBreak { span: underline });
                    }
                }
            }
            RawBlock::BlockQuote { span, children } => retained.push(RawBlock::BlockQuote {
                span,
                children: extract_definitions(source, children, definitions, footnotes),
            }),
            RawBlock::List {
                span,
                kind,
                tight,
                items,
            } => retained.push(RawBlock::List {
                span,
                kind,
                tight,
                items: items
                    .into_iter()
                    .map(|mut item| {
                        item.task = strip_task_marker(source, &mut item.children);
                        RawListItem {
                            span: item.span,
                            task: item.task,
                            children: extract_definitions(
                                source,
                                item.children,
                                definitions,
                                footnotes,
                            ),
                        }
                    })
                    .collect(),
            }),
            other => retained.push(other),
        }
    }
    retained
}

fn setext_underline_span(source: &str, heading: Span, level: u8) -> Span {
    let marker = if level == 1 { b'=' } else { b'-' };
    let bytes = heading.slice(source).as_bytes();
    let mut marker_end = bytes.len();
    while marker_end > 0 && matches!(bytes[marker_end - 1], b' ' | b'\t') {
        marker_end -= 1;
    }
    let mut marker_start = marker_end;
    while marker_start > 0 && bytes[marker_start - 1] == marker {
        marker_start -= 1;
    }
    debug_assert!(marker_start < marker_end);
    Span {
        start: heading.start + marker_start as u32,
        end: heading.end,
    }
}

fn starts_on_next_line(source: &str, previous_end: u32, next_start: u32) -> bool {
    let Some(gap) = source
        .as_bytes()
        .get(previous_end as usize..next_start as usize)
    else {
        return false;
    };
    let Some(line_ending) = memchr2(b'\r', b'\n', gap) else {
        return false;
    };
    let next_line = if gap[line_ending] == b'\r' && gap.get(line_ending + 1) == Some(&b'\n') {
        line_ending + 2
    } else {
        line_ending + 1
    };
    memchr2(b'\r', b'\n', &gap[next_line..]).is_none()
}

fn extract_leading_link_definitions(
    source: &str,
    fragments: &[Span],
    definitions: &mut Vec<LinkDefinition>,
) -> usize {
    let mut consumed = 0;
    while let Some((definition, definition_fragments)) =
        parse_link_definition(source, &fragments[consumed..])
    {
        definitions.push(definition);
        consumed += definition_fragments;
    }
    consumed
}

fn strip_task_marker(source: &str, children: &mut [RawBlock]) -> Option<bool> {
    let RawBlock::Paragraph { span, fragments } = children.first_mut()? else {
        return None;
    };
    let first = fragments.first_mut()?;
    let text = first.slice(source);
    let checked = match text.as_bytes().get(..3)? {
        b"[ ]" => false,
        b"[x]" | b"[X]" => true,
        _ => return None,
    };
    if text.len() > 3 && !matches!(text.as_bytes()[3], b' ' | b'\t') {
        return None;
    }
    let whitespace = text.as_bytes()[3..]
        .iter()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count() as u32;
    first.start += 3 + whitespace;
    span.start = first.start;
    Some(checked)
}

fn parse_footnote_definition(
    source: &str,
    span: Span,
    fragments: &mut Vec<Span>,
) -> Option<(Span, RawBlock)> {
    let first = *fragments.first()?;
    let text = first.slice(source);
    if !text.starts_with("[^") {
        return None;
    }
    let close = text.find(']')?;
    let label_text = &text[2..close];
    if label_text.is_empty()
        || label_text
            .chars()
            .any(|character| character.is_whitespace() || matches!(character, '[' | ']'))
        || text.as_bytes().get(close + 1) != Some(&b':')
    {
        return None;
    }
    let whitespace_start = close + 2;
    if !matches!(text.as_bytes().get(whitespace_start), Some(b' ' | b'\t')) {
        return None;
    }
    let whitespace = text.as_bytes()[whitespace_start..]
        .iter()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count();
    let content_start = first.start + (whitespace_start + whitespace) as u32;
    fragments[0].start = content_start;
    let label = Span {
        start: first.start + 2,
        end: first.start + close as u32,
    };
    let content = RawBlock::Paragraph {
        span: Span {
            start: content_start,
            end: span.end,
        },
        fragments: std::mem::take(fragments),
    };
    // TODO(inline): attach four-space-indented continuation blocks.
    Some((
        label,
        RawBlock::FootnoteDefinition {
            span,
            label,
            children: vec![content],
        },
    ))
}

#[derive(Debug)]
enum Atom {
    Inline(Inline),
    Bracket {
        span: Span,
    },
    Delimiter {
        kind: DelimiterKind,
        span: Span,
        len: u32,
        can_open: bool,
        can_close: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DelimiterKind {
    Asterisk,
    Underscore,
    Strikethrough,
    Highlight,
}

#[derive(Clone, Copy, Debug)]
struct Bracket {
    atom_index: usize,
    image: bool,
    active: bool,
}

struct TargetMatch {
    target: LinkTarget,
    fragment_index: usize,
    end: u32,
}

fn parse_fragments(
    source: &str,
    fragments: &[Span],
    definitions: &[LinkDefinition],
    references: &HashMap<String, usize>,
    footnotes: &HashSet<String>,
) -> Vec<Inline> {
    let mut atoms = Vec::new();
    let mut brackets = Vec::new();
    let mut fragment_index = 0;
    let mut position = fragments.first().map_or(0, |fragment| fragment.start);

    'fragments: while fragment_index < fragments.len() {
        let fragment = fragments[fragment_index];
        position = position.max(fragment.start);
        let (scan_end, break_node) =
            fragment_end(source, fragment, fragment_index + 1 == fragments.len());
        let mut text_start = position;

        while position < scan_end {
            if let Some((end, email)) =
                parse_literal_autolink(source, position, scan_end, fragment.start)
            {
                push_text_atom(
                    &mut atoms,
                    Span {
                        start: text_start,
                        end: position,
                    },
                );
                let span = Span {
                    start: position,
                    end,
                };
                atoms.push(Atom::Inline(Inline::Autolink {
                    span,
                    uri: span,
                    email,
                }));
                position = end;
                text_start = position;
                continue;
            }
            let Some(character) = source[position as usize..scan_end as usize].chars().next()
            else {
                break;
            };

            match character {
                '\\' => {
                    let next_start = position + 1;
                    let next = source[next_start as usize..scan_end as usize]
                        .chars()
                        .next();
                    if next.is_some_and(|next| next.is_ascii_punctuation()) {
                        push_text_atom(
                            &mut atoms,
                            Span {
                                start: text_start,
                                end: position,
                            },
                        );
                        let end = next_start + next.map_or(0, |next| next.len_utf8() as u32);
                        atoms.push(Atom::Inline(Inline::Escaped {
                            span: Span {
                                start: position,
                                end,
                            },
                        }));
                        position = end;
                        text_start = position;
                    } else {
                        position += 1;
                    }
                }
                '&' => {
                    if let Some((end, value)) = character_reference(source, position, scan_end) {
                        push_text_atom(
                            &mut atoms,
                            Span {
                                start: text_start,
                                end: position,
                            },
                        );
                        atoms.push(Atom::Inline(Inline::CharacterReference {
                            span: Span {
                                start: position,
                                end,
                            },
                            value,
                        }));
                        position = end;
                        text_start = position;
                    } else {
                        position += 1;
                    }
                }
                '`' => {
                    let run_len = ascii_run_len(source, position, fragment.end, b'`');
                    if let Some(closer) = find_code_closer(
                        source,
                        fragments,
                        fragment_index,
                        position + run_len,
                        run_len,
                    ) {
                        push_text_atom(
                            &mut atoms,
                            Span {
                                start: text_start,
                                end: position,
                            },
                        );
                        let literal = code_literal(
                            source,
                            fragments,
                            fragment_index,
                            position + run_len,
                            closer.fragment_index,
                            closer.start,
                        );
                        atoms.push(Atom::Inline(Inline::CodeSpan {
                            span: Span {
                                start: position,
                                end: closer.start + run_len,
                            },
                            literal,
                        }));
                        let crossed_fragment = closer.fragment_index != fragment_index;
                        fragment_index = closer.fragment_index;
                        position = closer.start + run_len;
                        text_start = position;
                        if crossed_fragment {
                            continue 'fragments;
                        }
                    } else {
                        position += run_len;
                    }
                }
                '$' => {
                    let available_run = ascii_run_len(source, position, scan_end, b'$');
                    let run_len = if available_run >= 2 { 2 } else { 1 };
                    let can_open = run_len == 2
                        || next_char(source, position + 1, scan_end)
                            .is_some_and(|next| !next.is_whitespace());
                    if can_open
                        && let Some(closer) = find_math_closer(
                            source,
                            fragments,
                            fragment_index,
                            position + run_len,
                            run_len,
                        )
                    {
                        push_text_atom(
                            &mut atoms,
                            Span {
                                start: text_start,
                                end: position,
                            },
                        );
                        let literal = literal_spans(
                            fragments,
                            fragment_index,
                            position + run_len,
                            closer.fragment_index,
                            closer.start,
                        );
                        atoms.push(Atom::Inline(Inline::Math {
                            span: Span {
                                start: position,
                                end: closer.start + run_len,
                            },
                            display: run_len == 2,
                            literal,
                        }));
                        let crossed_fragment = closer.fragment_index != fragment_index;
                        fragment_index = closer.fragment_index;
                        position = closer.start + run_len;
                        text_start = position;
                        if crossed_fragment {
                            continue 'fragments;
                        }
                    } else {
                        position += run_len;
                    }
                }
                '<' => {
                    if let Some((end, uri, email)) = parse_autolink(source, position, scan_end) {
                        push_text_atom(
                            &mut atoms,
                            Span {
                                start: text_start,
                                end: position,
                            },
                        );
                        atoms.push(Atom::Inline(Inline::Autolink {
                            span: Span {
                                start: position,
                                end,
                            },
                            uri,
                            email,
                        }));
                        position = end;
                        text_start = position;
                    } else {
                        position += 1;
                    }
                }
                '!' if source[position as usize..scan_end as usize].starts_with("![") => {
                    push_text_atom(
                        &mut atoms,
                        Span {
                            start: text_start,
                            end: position,
                        },
                    );
                    let end = position + 2;
                    brackets.push(Bracket {
                        atom_index: atoms.len(),
                        image: true,
                        active: true,
                    });
                    atoms.push(Atom::Bracket {
                        span: Span {
                            start: position,
                            end,
                        },
                    });
                    position = end;
                    text_start = position;
                }
                '[' if parse_footnote_reference(source, position, scan_end, footnotes)
                    .is_some() =>
                {
                    let (end, label) =
                        parse_footnote_reference(source, position, scan_end, footnotes)
                            .expect("footnote reference was checked by the match guard");
                    push_text_atom(
                        &mut atoms,
                        Span {
                            start: text_start,
                            end: position,
                        },
                    );
                    atoms.push(Atom::Inline(Inline::FootnoteReference {
                        span: Span {
                            start: position,
                            end,
                        },
                        label,
                    }));
                    position = end;
                    text_start = position;
                }
                '[' if parse_wikilink(source, position, scan_end).is_some() => {
                    let (end, target, label) = parse_wikilink(source, position, scan_end)
                        .expect("wikilink was checked by the match guard");
                    push_text_atom(
                        &mut atoms,
                        Span {
                            start: text_start,
                            end: position,
                        },
                    );
                    atoms.push(Atom::Inline(Inline::WikiLink {
                        span: Span {
                            start: position,
                            end,
                        },
                        target,
                        label,
                    }));
                    position = end;
                    text_start = position;
                }
                '[' => {
                    push_text_atom(
                        &mut atoms,
                        Span {
                            start: text_start,
                            end: position,
                        },
                    );
                    let end = position + 1;
                    brackets.push(Bracket {
                        atom_index: atoms.len(),
                        image: false,
                        active: true,
                    });
                    atoms.push(Atom::Bracket {
                        span: Span {
                            start: position,
                            end,
                        },
                    });
                    position = end;
                    text_start = position;
                }
                ']' => {
                    push_text_atom(
                        &mut atoms,
                        Span {
                            start: text_start,
                            end: position,
                        },
                    );
                    let Some(bracket_index) = brackets.iter().rposition(|bracket| bracket.active)
                    else {
                        position += 1;
                        text_start = position - 1;
                        continue;
                    };
                    let bracket = brackets[bracket_index];
                    let opening_span = match atoms[bracket.atom_index] {
                        Atom::Bracket { span } => span,
                        _ => unreachable!("bracket stack points at a non-bracket atom"),
                    };
                    let Some(target_match) = parse_inline_target(source, position + 1, scan_end)
                        .map(|(target, end)| TargetMatch {
                            target,
                            fragment_index,
                            end,
                        })
                        .or_else(|| {
                            parse_multiline_inline_target(
                                source,
                                fragments,
                                fragment_index,
                                position + 1,
                                scan_end,
                            )
                        })
                        .or_else(|| {
                            parse_reference_target(
                                source,
                                opening_span.end,
                                position,
                                scan_end,
                                definitions,
                                references,
                            )
                            .map(|(target, end)| TargetMatch {
                                target,
                                fragment_index,
                                end,
                            })
                        })
                    else {
                        brackets.truncate(bracket_index);
                        position += 1;
                        text_start = position - 1;
                        continue;
                    };
                    let mut children_atoms = atoms.drain(bracket.atom_index + 1..).collect();
                    process_delimiters(&mut children_atoms);
                    let children = atoms_to_inlines(children_atoms);
                    atoms.pop();
                    let span = Span {
                        start: opening_span.start,
                        end: target_match.end,
                    };
                    let inline = if bracket.image {
                        Inline::Image {
                            span,
                            target: target_match.target,
                            children,
                        }
                    } else {
                        Inline::Link {
                            span,
                            target: target_match.target,
                            children,
                        }
                    };
                    atoms.push(Atom::Inline(inline));
                    brackets.truncate(bracket_index);
                    if !bracket.image {
                        for earlier in &mut brackets {
                            if !earlier.image {
                                earlier.active = false;
                            }
                        }
                    }
                    let crossed_fragment = target_match.fragment_index != fragment_index;
                    fragment_index = target_match.fragment_index;
                    position = target_match.end;
                    text_start = position;
                    if crossed_fragment {
                        continue 'fragments;
                    }
                }
                '*' | '_' => {
                    push_text_atom(
                        &mut atoms,
                        Span {
                            start: text_start,
                            end: position,
                        },
                    );
                    let byte = character as u8;
                    let run_len = ascii_run_len(source, position, fragment.end, byte);
                    let run_end = position + run_len;
                    let previous = if position == fragment.start {
                        None
                    } else {
                        source[fragment.start as usize..position as usize]
                            .chars()
                            .next_back()
                    };
                    let next = if run_end == fragment.end {
                        None
                    } else {
                        source[run_end as usize..fragment.end as usize]
                            .chars()
                            .next()
                    };
                    let (can_open, can_close) = delimiter_flanking(character, previous, next);
                    atoms.push(Atom::Delimiter {
                        kind: if character == '*' {
                            DelimiterKind::Asterisk
                        } else {
                            DelimiterKind::Underscore
                        },
                        span: Span {
                            start: position,
                            end: run_end,
                        },
                        len: run_len,
                        can_open,
                        can_close,
                    });
                    position = run_end;
                    text_start = position;
                }
                '~' | '=' => {
                    let byte = character as u8;
                    let run_len = ascii_run_len(source, position, fragment.end, byte);
                    let participates = if character == '~' {
                        run_len <= 2
                    } else {
                        run_len == 2
                    };
                    if !participates {
                        position += run_len;
                        continue;
                    }
                    push_text_atom(
                        &mut atoms,
                        Span {
                            start: text_start,
                            end: position,
                        },
                    );
                    let run_end = position + run_len;
                    let previous = if position == fragment.start {
                        None
                    } else {
                        source[fragment.start as usize..position as usize]
                            .chars()
                            .next_back()
                    };
                    let next = if run_end == fragment.end {
                        None
                    } else {
                        source[run_end as usize..fragment.end as usize]
                            .chars()
                            .next()
                    };
                    let (can_open, can_close) = delimiter_flanking('*', previous, next);
                    atoms.push(Atom::Delimiter {
                        kind: if character == '~' {
                            DelimiterKind::Strikethrough
                        } else {
                            DelimiterKind::Highlight
                        },
                        span: Span {
                            start: position,
                            end: run_end,
                        },
                        len: run_len,
                        can_open,
                        can_close,
                    });
                    position = run_end;
                    text_start = position;
                }
                _ => position += character.len_utf8() as u32,
            }
        }

        push_text_atom(
            &mut atoms,
            Span {
                start: text_start,
                end: scan_end,
            },
        );
        if let Some(node) = break_node {
            atoms.push(Atom::Inline(node));
        }
        fragment_index += 1;
        if let Some(fragment) = fragments.get(fragment_index) {
            position = fragment.start;
        }
    }

    process_delimiters(&mut atoms);
    atoms_to_inlines(atoms)
}

fn fragment_end(source: &str, fragment: Span, final_fragment: bool) -> (u32, Option<Inline>) {
    let text = fragment.slice(source);
    let trimmed = text.trim_end_matches([' ', '\t']);
    let trimmed_end = fragment.start + trimmed.len() as u32;
    if final_fragment {
        return (trimmed_end, None);
    }

    let trailing = &source[trimmed_end as usize..fragment.end as usize];
    if trailing.len() >= 2 && trailing.bytes().all(|byte| byte == b' ') {
        return (
            trimmed_end,
            Some(Inline::HardBreak {
                span: Span {
                    start: trimmed_end,
                    end: fragment.end,
                },
            }),
        );
    }
    // Only an ODD number of trailing backslashes leaves a real `\` before the
    // line ending — an even count means the last backslash is itself escaped
    // (a literal `\`), which cannot form a hard break.
    let trailing_backslashes = trimmed
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\\')
        .count();
    if trailing.is_empty() && trailing_backslashes % 2 == 1 {
        let slash_start = trimmed_end - 1;
        return (
            slash_start,
            Some(Inline::HardBreak {
                span: Span {
                    start: slash_start,
                    end: trimmed_end,
                },
            }),
        );
    }
    (
        trimmed_end,
        Some(Inline::SoftBreak {
            span: Span {
                start: trimmed_end,
                end: trimmed_end,
            },
        }),
    )
}

fn parse_inline_target(source: &str, start: u32, end: u32) -> Option<(LinkTarget, u32)> {
    if source.as_bytes().get(start as usize) != Some(&b'(') {
        return None;
    }
    let mut position = skip_link_whitespace(source, start + 1, end);
    let dest = if source.as_bytes().get(position as usize) == Some(&b'<') {
        let content_start = position + 1;
        position = content_start;
        loop {
            let character = next_char(source, position, end)?;
            match character {
                '\\' => {
                    position += 1;
                    position += next_char(source, position, end)?.len_utf8() as u32;
                }
                '<' | '\n' | '\r' => return None,
                '>' => break,
                _ => position += character.len_utf8() as u32,
            }
        }
        let span = Span {
            start: content_start,
            end: position,
        };
        position += 1;
        (span.start < span.end).then_some(span)
    } else {
        let content_start = position;
        let mut depth = 0_u32;
        while position < end {
            let character = next_char(source, position, end)?;
            match character {
                '\\' => {
                    position += 1;
                    position += next_char(source, position, end)?.len_utf8() as u32;
                }
                '(' => {
                    depth += 1;
                    position += 1;
                }
                ')' if depth > 0 => {
                    depth -= 1;
                    position += 1;
                }
                ')' => break,
                character if character.is_whitespace() || character.is_control() => break,
                _ => position += character.len_utf8() as u32,
            }
        }
        if depth != 0 {
            return None;
        }
        let span = Span {
            start: content_start,
            end: position,
        };
        (span.start < span.end).then_some(span)
    };

    let before_whitespace = position;
    position = skip_link_whitespace(source, position, end);
    let had_whitespace = before_whitespace != position;
    let title = if had_whitespace {
        parse_link_title(source, position, end).map(|(title, title_end)| {
            position = title_end;
            title
        })
    } else {
        None
    };
    position = skip_link_whitespace(source, position, end);
    if source.as_bytes().get(position as usize) != Some(&b')') {
        return None;
    }

    // Destination and title stay raw; consumers decide how to interpret
    // escapes and character references in these source-backed spans.
    Some((
        LinkTarget {
            dest,
            title,
            label: None,
        },
        position + 1,
    ))
}

fn parse_multiline_inline_target(
    source: &str,
    fragments: &[Span],
    opening_fragment: usize,
    start: u32,
    opening_end: u32,
) -> Option<TargetMatch> {
    if source.as_bytes().get(start as usize) != Some(&b'(') {
        return None;
    }
    let mut position = skip_link_whitespace(source, start + 1, opening_end);
    let dest = if source.as_bytes().get(position as usize) == Some(&b'<') {
        let content_start = position + 1;
        position = content_start;
        loop {
            let character = next_char(source, position, opening_end)?;
            match character {
                '\\' => {
                    position += 1;
                    position += next_char(source, position, opening_end)?.len_utf8() as u32;
                }
                '<' | '\n' | '\r' => return None,
                '>' => break,
                _ => position += character.len_utf8() as u32,
            }
        }
        let span = Span {
            start: content_start,
            end: position,
        };
        position += 1;
        (span.start < span.end).then_some(span)
    } else {
        let content_start = position;
        let mut depth = 0_u32;
        while position < opening_end {
            let character = next_char(source, position, opening_end)?;
            match character {
                '\\' => {
                    position += 1;
                    position += next_char(source, position, opening_end)?.len_utf8() as u32;
                }
                '(' => {
                    depth += 1;
                    position += 1;
                }
                ')' if depth > 0 => {
                    depth -= 1;
                    position += 1;
                }
                ')' => break,
                character if character.is_whitespace() || character.is_control() => break,
                _ => position += character.len_utf8() as u32,
            }
        }
        if depth != 0 {
            return None;
        }
        let span = Span {
            start: content_start,
            end: position,
        };
        (span.start < span.end).then_some(span)
    };

    let before_whitespace = position;
    position = skip_link_whitespace(source, position, opening_end);
    if position == before_whitespace {
        return None;
    }
    let opening = next_char(source, position, opening_end)?;
    let closing = match opening {
        '"' => '"',
        '\'' => '\'',
        '(' => ')',
        _ => return None,
    };
    let title_start = position + 1;
    position = title_start;

    for (fragment_index, fragment) in fragments.iter().enumerate().skip(opening_fragment) {
        let fragment_end = if fragment_index == opening_fragment {
            opening_end
        } else {
            fragment.end
        };
        if fragment_index != opening_fragment {
            position = fragment.start;
        }
        while position < fragment_end {
            let character = next_char(source, position, fragment_end)?;
            if character == '\\' {
                position += 1;
                if let Some(escaped) = next_char(source, position, fragment_end) {
                    position += escaped.len_utf8() as u32;
                }
            } else if character == closing {
                let title = Span {
                    start: title_start,
                    end: position,
                };
                position += 1;
                position = skip_link_whitespace(source, position, fragment_end);
                if source.as_bytes().get(position as usize) != Some(&b')') {
                    return None;
                }
                return Some(TargetMatch {
                    target: LinkTarget {
                        dest,
                        title: Some(title),
                        label: None,
                    },
                    fragment_index,
                    end: position + 1,
                });
            } else {
                position += character.len_utf8() as u32;
            }
        }
    }
    None
}

fn parse_reference_target(
    source: &str,
    own_label_start: u32,
    close: u32,
    end: u32,
    definitions: &[LinkDefinition],
    references: &HashMap<String, usize>,
) -> Option<(LinkTarget, u32)> {
    let own_label = Span {
        start: own_label_start,
        end: close,
    };
    let (used_label, syntax_end) = if source.as_bytes().get(close as usize + 1) == Some(&b'[') {
        let label_start = close + 2;
        if let Some(label_end) = find_reference_label_end(source, label_start, end) {
            let explicit = Span {
                start: label_start,
                end: label_end,
            };
            (
                if explicit.start == explicit.end {
                    own_label
                } else {
                    explicit
                },
                label_end + 1,
            )
        } else {
            (own_label, close + 1)
        }
    } else {
        (own_label, close + 1)
    };
    let definition = definitions[*references.get(&normalize_label(used_label.slice(source)))?];
    Some((
        LinkTarget {
            dest: Some(definition.dest),
            title: definition.title,
            label: Some(used_label),
        },
        syntax_end,
    ))
}

fn find_reference_label_end(source: &str, mut position: u32, end: u32) -> Option<u32> {
    while position < end {
        let character = next_char(source, position, end)?;
        match character {
            '\\' => {
                position += 1;
                position += next_char(source, position, end)?.len_utf8() as u32;
            }
            ']' => return Some(position),
            '[' => return None,
            _ => position += character.len_utf8() as u32,
        }
    }
    None
}

#[derive(Clone, Copy)]
struct LinkDefinitionCursor<'a> {
    source: &'a str,
    fragments: &'a [Span],
    fragment_index: usize,
    position: u32,
}

impl<'a> LinkDefinitionCursor<'a> {
    fn new(source: &'a str, fragments: &'a [Span]) -> Option<Self> {
        Some(Self {
            source,
            fragments,
            fragment_index: 0,
            position: fragments.first()?.start,
        })
    }

    fn fragment(self) -> Span {
        self.fragments[self.fragment_index]
    }

    fn current_char(self) -> Option<char> {
        next_char(self.source, self.position, self.fragment().end)
    }

    fn bump_char(&mut self) -> Option<char> {
        let character = self.current_char()?;
        self.position += character.len_utf8() as u32;
        Some(character)
    }

    fn at_fragment_end(self) -> bool {
        self.position == self.fragment().end
    }

    fn advance_fragment(&mut self) -> bool {
        let next_index = self.fragment_index + 1;
        let Some(next) = self.fragments.get(next_index) else {
            return false;
        };
        self.fragment_index = next_index;
        self.position = next.start;
        true
    }

    fn skip_spaces_and_tabs(&mut self) {
        while matches!(self.current_char(), Some(' ' | '\t')) {
            self.bump_char();
        }
    }

    fn consumed_fragments(self) -> usize {
        self.fragment_index + 1
    }
}

fn parse_link_definition(source: &str, fragments: &[Span]) -> Option<(LinkDefinition, usize)> {
    let mut cursor = LinkDefinitionCursor::new(source, fragments)?;
    let definition_start = cursor.fragment().start;
    if cursor.bump_char()? != '[' {
        return None;
    }

    let label_start = cursor.position;
    let mut label_len = 0;
    let mut label_has_non_whitespace = false;
    let label_end = loop {
        if cursor.at_fragment_end() {
            if !cursor.advance_fragment() {
                return None;
            }
            label_len += 1;
            if label_len > 999 {
                return None;
            }
            continue;
        }

        let character_start = cursor.position;
        let character = cursor.bump_char()?;
        if character == ']' {
            break character_start;
        }
        if character == '[' {
            return None;
        }

        label_len += 1;
        if label_len > 999 {
            return None;
        }
        label_has_non_whitespace |= !character.is_whitespace();
        if character == '\\' && !cursor.at_fragment_end() {
            let escaped = cursor.bump_char()?;
            label_len += 1;
            if label_len > 999 {
                return None;
            }
            label_has_non_whitespace |= !escaped.is_whitespace();
        }
    };
    if !label_has_non_whitespace || cursor.bump_char()? != ':' {
        return None;
    }
    let label = Span {
        start: label_start,
        end: label_end,
    };

    cursor.skip_spaces_and_tabs();
    if cursor.at_fragment_end() {
        cursor.advance_fragment().then_some(())?;
        cursor.skip_spaces_and_tabs();
    }
    let fragment_end = cursor.fragment().end;
    let (dest, dest_end) = parse_definition_destination(source, cursor.position, fragment_end)?;
    cursor.position = dest_end;

    let before_whitespace = cursor.position;
    cursor.skip_spaces_and_tabs();
    if !cursor.at_fragment_end() {
        if cursor.position == before_whitespace {
            return None;
        }
        let title = parse_multiline_definition_title(&mut cursor)?;
        cursor.skip_spaces_and_tabs();
        if !cursor.at_fragment_end() {
            return None;
        }
        let definition = LinkDefinition {
            span: Span {
                start: definition_start,
                end: cursor.fragment().end,
            },
            label,
            dest,
            title: Some(title),
        };
        return Some((definition, cursor.consumed_fragments()));
    }

    let destination_fragment = cursor.fragment();
    let destination_fragment_count = cursor.consumed_fragments();
    let mut title_cursor = cursor;
    if title_cursor.advance_fragment() {
        title_cursor.skip_spaces_and_tabs();
        if matches!(title_cursor.current_char(), Some('"' | '\'' | '('))
            && let Some(title) = parse_multiline_definition_title(&mut title_cursor)
        {
            title_cursor.skip_spaces_and_tabs();
            if title_cursor.at_fragment_end() {
                let definition = LinkDefinition {
                    span: Span {
                        start: definition_start,
                        end: title_cursor.fragment().end,
                    },
                    label,
                    dest,
                    title: Some(title),
                };
                return Some((definition, title_cursor.consumed_fragments()));
            }
        }
    }

    Some((
        LinkDefinition {
            span: Span {
                start: definition_start,
                end: destination_fragment.end,
            },
            label,
            dest,
            title: None,
        },
        destination_fragment_count,
    ))
}

fn parse_multiline_definition_title(cursor: &mut LinkDefinitionCursor<'_>) -> Option<Span> {
    let opening = cursor.bump_char()?;
    let closing = match opening {
        '"' => '"',
        '\'' => '\'',
        '(' => ')',
        _ => return None,
    };
    let title_start = cursor.position;
    loop {
        if cursor.at_fragment_end() {
            cursor.advance_fragment().then_some(())?;
            continue;
        }
        let character_start = cursor.position;
        let character = cursor.bump_char()?;
        if character == '\\' && !cursor.at_fragment_end() {
            cursor.bump_char()?;
        } else if opening == '(' && character == '(' {
            return None;
        } else if character == closing {
            return Some(Span {
                start: title_start,
                end: character_start,
            });
        }
    }
}

fn parse_autolink(source: &str, start: u32, end: u32) -> Option<(u32, Span, bool)> {
    let content_start = start + 1;
    let candidate = &source[content_start as usize..end as usize];
    let closing = memchr(b'>', candidate.as_bytes())? as u32 + content_start;
    let uri = Span {
        start: content_start,
        end: closing,
    };
    let text = uri.slice(source);
    if text.is_empty()
        || text
            .chars()
            .any(|character| character.is_whitespace() || character == '<')
    {
        return None;
    }
    let email = if valid_uri_autolink(text) {
        false
    } else if valid_email_autolink(text) {
        true
    } else {
        return None;
    };
    Some((closing + 1, uri, email))
}

fn parse_literal_autolink(
    source: &str,
    start: u32,
    scan_end: u32,
    fragment_start: u32,
) -> Option<(u32, bool)> {
    if start != fragment_start {
        let previous = source[fragment_start as usize..start as usize]
            .chars()
            .next_back()?;
        if !previous.is_whitespace() && !matches!(previous, '*' | '_' | '~' | '(') {
            return None;
        }
    }

    let remaining = &source[start as usize..scan_end as usize];
    if remaining.starts_with("www.") {
        let end = trim_literal_autolink(
            source,
            start,
            literal_candidate_end(source, start, scan_end),
        );
        return valid_literal_url(source, start, end, 0).then_some((end, false));
    }
    let scheme_len = if remaining.starts_with("https://") {
        8
    } else if remaining.starts_with("http://") {
        7
    } else {
        0
    };
    if scheme_len != 0 {
        let end = trim_literal_autolink(
            source,
            start,
            literal_candidate_end(source, start, scan_end),
        );
        return valid_literal_url(source, start, end, scheme_len).then_some((end, false));
    }

    let local_len = remaining
        .bytes()
        .take_while(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-')
        })
        .count();
    if local_len == 0 || remaining.as_bytes().get(local_len) != Some(&b'@') {
        return None;
    }
    let domain_start = start + local_len as u32 + 1;
    let domain_end = scan_domain_end(source, domain_start, scan_end);
    let end = trim_literal_autolink(source, start, domain_end);
    valid_literal_email(source, start, domain_start, end).then_some((end, true))
}

fn literal_candidate_end(source: &str, mut position: u32, scan_end: u32) -> u32 {
    while position < scan_end {
        let Some(character) = next_char(source, position, scan_end) else {
            break;
        };
        if character.is_whitespace() || matches!(character, '<' | '>') {
            break;
        }
        position += character.len_utf8() as u32;
    }
    position
}

fn trim_literal_autolink(source: &str, start: u32, mut end: u32) -> u32 {
    while let Some(character) = source[start as usize..end as usize].chars().next_back() {
        if matches!(character, '?' | '!' | '.' | ',' | ':' | '*' | '_' | '~') {
            end -= character.len_utf8() as u32;
            continue;
        }
        if character == ')' {
            let candidate = &source[start as usize..end as usize];
            if candidate.matches(')').count() > candidate.matches('(').count() {
                end -= 1;
                continue;
            }
        }
        // TODO(inline): strip a complete entity-like tail ending in `;`.
        break;
    }
    end
}

fn valid_literal_url(source: &str, start: u32, end: u32, scheme_len: u32) -> bool {
    let domain_start = start + scheme_len;
    let domain_end = scan_domain_end(source, domain_start, end);
    if !valid_literal_domain(&source[domain_start as usize..domain_end as usize]) {
        return false;
    }
    domain_end == end
        || source
            .as_bytes()
            .get(domain_end as usize)
            .is_some_and(|byte| matches!(byte, b'/' | b'?' | b'#'))
}

fn scan_domain_end(source: &str, mut position: u32, end: u32) -> u32 {
    while position < end {
        let byte = source.as_bytes()[position as usize];
        if !byte.is_ascii_alphanumeric() && !matches!(byte, b'-' | b'_' | b'.') {
            break;
        }
        position += 1;
    }
    position
}

fn valid_literal_domain(domain: &str) -> bool {
    let segments = domain.split('.').collect::<Vec<_>>();
    if segments.len() < 2
        || segments.iter().any(|segment| {
            segment.is_empty()
                || matches!(segment.as_bytes().first(), Some(b'-' | b'_'))
                || matches!(segment.as_bytes().last(), Some(b'-' | b'_'))
        })
    {
        return false;
    }
    !segments[segments.len() - 2..]
        .iter()
        .any(|segment| segment.contains('_'))
}

fn valid_literal_email(source: &str, start: u32, domain_start: u32, end: u32) -> bool {
    if domain_start >= end {
        return false;
    }
    let local_end = domain_start - 1;
    let local = &source[start as usize..local_end as usize];
    let domain = &source[domain_start as usize..end as usize];
    !local.is_empty()
        && local.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-')
        })
        && valid_literal_domain(domain)
        && domain
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphabetic)
}

fn parse_footnote_reference(
    source: &str,
    start: u32,
    end: u32,
    footnotes: &HashSet<String>,
) -> Option<(u32, Span)> {
    let candidate = &source[start as usize..end as usize];
    if !candidate.starts_with("[^") {
        return None;
    }
    let close = candidate.find(']')?;
    let label = Span {
        start: start + 2,
        end: start + close as u32,
    };
    let text = label.slice(source);
    if text.is_empty()
        || text
            .chars()
            .any(|character| character.is_whitespace() || matches!(character, '[' | ']'))
        || !footnotes.contains(&normalize_label(text))
    {
        return None;
    }
    Some((start + close as u32 + 1, label))
}

fn parse_wikilink(source: &str, start: u32, end: u32) -> Option<(u32, Span, Option<Span>)> {
    let content_start = start + 2;
    if !source[start as usize..end as usize].starts_with("[[") {
        return None;
    }
    let content = &source[content_start as usize..end as usize];
    let close = content.find("]]")? as u32 + content_start;
    let separator = source[content_start as usize..close as usize]
        .find('|')
        .map(|offset| content_start + offset as u32);
    let target = Span {
        start: content_start,
        end: separator.unwrap_or(close),
    };
    if target.start == target.end {
        return None;
    }
    let label = separator.map(|separator| Span {
        start: separator + 1,
        end: close,
    });
    Some((close + 2, target, label))
}

fn valid_uri_autolink(candidate: &str) -> bool {
    let Some(colon) = candidate.find(':') else {
        return false;
    };
    let scheme = &candidate[..colon];
    (2..=32).contains(&scheme.len())
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

fn valid_email_autolink(candidate: &str) -> bool {
    let Some((local, domain)) = candidate.split_once('@') else {
        return false;
    };
    if local.is_empty()
        || domain.is_empty()
        || candidate.matches('@').count() != 1
        || !local.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'.' | b'!'
                        | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'/'
                        | b'='
                        | b'?'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'{'
                        | b'|'
                        | b'}'
                        | b'~'
                )
        })
    {
        return false;
    }
    let mut labels = domain.split('.').peekable();
    if labels.peek().is_none() || !domain.contains('.') {
        return false;
    }
    labels.all(|label| {
        !label.is_empty()
            && label.as_bytes()[0].is_ascii_alphanumeric()
            && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

fn parse_definition_destination(source: &str, start: u32, end: u32) -> Option<(Span, u32)> {
    if source.as_bytes().get(start as usize) == Some(&b'<') {
        let content_start = start + 1;
        let mut position = content_start;
        while position < end {
            let character = next_char(source, position, end)?;
            match character {
                '\\' => {
                    position += 1;
                    position += next_char(source, position, end)?.len_utf8() as u32;
                }
                '<' => return None,
                '>' => {
                    return Some((
                        Span {
                            start: content_start,
                            end: position,
                        },
                        position + 1,
                    ));
                }
                _ => position += character.len_utf8() as u32,
            }
        }
        return None;
    }

    let mut position = start;
    let mut depth = 0_u32;
    while position < end {
        let character = next_char(source, position, end)?;
        match character {
            '\\' => {
                position += 1;
                position += next_char(source, position, end)?.len_utf8() as u32;
            }
            '(' => {
                depth += 1;
                position += 1;
            }
            ')' if depth > 0 => {
                depth -= 1;
                position += 1;
            }
            character if character.is_whitespace() || character.is_control() => break,
            _ => position += character.len_utf8() as u32,
        }
    }
    (position > start && depth == 0).then_some((
        Span {
            start,
            end: position,
        },
        position,
    ))
}

fn normalize_label(label: &str) -> String {
    let mut normalized = String::new();
    let mut pending_space = false;
    for character in label.trim().chars() {
        if character.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && !normalized.is_empty() {
            normalized.push(' ');
        }
        normalized.extend(character.to_lowercase());
        pending_space = false;
    }
    normalized
}

fn parse_link_title(source: &str, start: u32, end: u32) -> Option<(Span, u32)> {
    let opening = next_char(source, start, end)?;
    let closing = match opening {
        '"' => '"',
        '\'' => '\'',
        '(' => ')',
        _ => return None,
    };
    let content_start = start + 1;
    let mut position = content_start;
    while position < end {
        let character = next_char(source, position, end)?;
        if character == '\\' {
            position += 1;
            position += next_char(source, position, end)?.len_utf8() as u32;
        } else if character == closing {
            return Some((
                Span {
                    start: content_start,
                    end: position,
                },
                position + 1,
            ));
        } else {
            position += character.len_utf8() as u32;
        }
    }
    None
}

fn skip_link_whitespace(source: &str, mut position: u32, end: u32) -> u32 {
    while position < end {
        let Some(character) = next_char(source, position, end) else {
            break;
        };
        if !matches!(character, ' ' | '\t') {
            break;
        }
        position += character.len_utf8() as u32;
    }
    position
}

fn next_char(source: &str, position: u32, end: u32) -> Option<char> {
    source[position as usize..end as usize].chars().next()
}

fn character_reference(source: &str, start: u32, end: u32) -> Option<(u32, String)> {
    let candidate = &source[start as usize + 1..end as usize];
    let semicolon = memchr(b';', candidate.as_bytes())?;
    if semicolon > 32 {
        return None;
    }
    let body = &candidate[..semicolon];
    let value = if let Some(numeric) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
        decode_numeric(numeric, 16)?.to_string()
    } else if let Some(numeric) = body.strip_prefix('#') {
        decode_numeric(numeric, 10)?.to_string()
    } else {
        entities::lookup(body)?.to_owned()
    };
    Some((start + semicolon as u32 + 2, value))
}

pub(crate) fn decode_character_reference(reference: &str) -> Option<String> {
    let (end, value) = character_reference(reference, 0, reference.len() as u32)?;
    (end as usize == reference.len()).then_some(value)
}

fn decode_numeric(digits: &str, radix: u32) -> Option<char> {
    // Spec bounds: 1-7 decimal digits, 1-6 hex digits. Longer sequences are
    // not character references at all — they stay literal text.
    let max_digits = if radix == 16 { 6 } else { 7 };
    if digits.is_empty()
        || digits.len() > max_digits
        || !digits.is_ascii()
        || !digits.bytes().all(|byte| (byte as char).is_digit(radix))
    {
        return None;
    }
    let value = u64::from_str_radix(digits, radix).unwrap_or(u64::MAX);
    Some(if value == 0 || value > u32::MAX as u64 {
        '\u{fffd}'
    } else {
        char::from_u32(value as u32).unwrap_or('\u{fffd}')
    })
}

struct CodeCloser {
    fragment_index: usize,
    start: u32,
}

fn find_math_closer(
    source: &str,
    fragments: &[Span],
    opening_fragment: usize,
    after_opening: u32,
    run_len: u32,
) -> Option<CodeCloser> {
    for (fragment_index, fragment) in fragments.iter().enumerate().skip(opening_fragment) {
        let mut position = if fragment_index == opening_fragment {
            after_opening
        } else {
            fragment.start
        };
        while position < fragment.end {
            let haystack = &source[position as usize..fragment.end as usize];
            let Some(found) = memchr(b'$', haystack.as_bytes()) else {
                break;
            };
            position += found as u32;
            let found_len = ascii_run_len(source, position, fragment.end, b'$');
            if run_len == 2 && found_len >= 2 {
                return Some(CodeCloser {
                    fragment_index,
                    start: position,
                });
            }
            if run_len == 1 && found_len == 1 {
                let previous = source[fragment.start as usize..position as usize]
                    .chars()
                    .next_back();
                let next = next_char(source, position + 1, fragment.end);
                if previous.is_some_and(|previous| !previous.is_whitespace())
                    && !next.is_some_and(|next| next.is_ascii_digit())
                {
                    return Some(CodeCloser {
                        fragment_index,
                        start: position,
                    });
                }
            }
            position += found_len;
        }
    }
    None
}

fn literal_spans(
    fragments: &[Span],
    opening_fragment: usize,
    content_start: u32,
    closing_fragment: usize,
    content_end: u32,
) -> Vec<Span> {
    (opening_fragment..=closing_fragment)
        .map(|index| Span {
            start: if index == opening_fragment {
                content_start
            } else {
                fragments[index].start
            },
            end: if index == closing_fragment {
                content_end
            } else {
                fragments[index].end
            },
        })
        .collect()
}

fn find_code_closer(
    source: &str,
    fragments: &[Span],
    opening_fragment: usize,
    after_opening: u32,
    run_len: u32,
) -> Option<CodeCloser> {
    for (fragment_index, fragment) in fragments.iter().enumerate().skip(opening_fragment) {
        let mut position = if fragment_index == opening_fragment {
            after_opening
        } else {
            fragment.start
        };
        while position < fragment.end {
            let haystack = &source[position as usize..fragment.end as usize];
            let Some(found) = memchr(b'`', haystack.as_bytes()) else {
                break;
            };
            position += found as u32;
            let found_len = ascii_run_len(source, position, fragment.end, b'`');
            if found_len == run_len {
                return Some(CodeCloser {
                    fragment_index,
                    start: position,
                });
            }
            position += found_len;
        }
    }
    None
}

fn code_literal(
    source: &str,
    fragments: &[Span],
    opening_fragment: usize,
    content_start: u32,
    closing_fragment: usize,
    content_end: u32,
) -> Vec<Span> {
    let mut literal = (opening_fragment..=closing_fragment)
        .map(|index| Span {
            start: if index == opening_fragment {
                content_start
            } else {
                fragments[index].start
            },
            end: if index == closing_fragment {
                content_end
            } else {
                fragments[index].end
            },
        })
        .collect::<Vec<_>>();

    let all_spaces = literal
        .iter()
        .all(|span| span.slice(source).chars().all(|character| character == ' '));
    let first_space = literal
        .iter()
        .find(|span| span.start < span.end)
        .is_some_and(|span| span.slice(source).starts_with(' '));
    let last_space = literal
        .iter()
        .rev()
        .find(|span| span.start < span.end)
        .is_some_and(|span| span.slice(source).ends_with(' '));
    if first_space && last_space && !all_spaces {
        if let Some(first) = literal.iter_mut().find(|span| span.start < span.end) {
            first.start += 1;
        }
        if let Some(last) = literal.iter_mut().rev().find(|span| span.start < span.end) {
            last.end -= 1;
        }
    }
    literal
}

fn delimiter_flanking(character: char, previous: Option<char>, next: Option<char>) -> (bool, bool) {
    let previous_whitespace = previous.is_none_or(char::is_whitespace);
    let next_whitespace = next.is_none_or(char::is_whitespace);
    let previous_punctuation = previous.is_some_and(is_unicode_punctuation);
    let next_punctuation = next.is_some_and(is_unicode_punctuation);
    let left_flanking =
        !next_whitespace && (!next_punctuation || previous_whitespace || previous_punctuation);
    let right_flanking =
        !previous_whitespace && (!previous_punctuation || next_whitespace || next_punctuation);
    if character == '*' {
        (left_flanking, right_flanking)
    } else {
        (
            left_flanking && (!right_flanking || previous_punctuation),
            right_flanking && (!left_flanking || next_punctuation),
        )
    }
}

fn is_unicode_punctuation(character: char) -> bool {
    character.general_category_group() == GeneralCategoryGroup::Punctuation
}

fn process_delimiters(atoms: &mut Vec<Atom>) {
    let mut emphasis_bottom = [[[0; 3]; 2]; 2];
    // Indexed by [kind][run_len - 1]: a failed length-1 tilde closer says
    // nothing about length-2 openers, so the bottoms must not be shared
    // across run lengths (extensions pair on exact length only).
    let mut extension_bottom = [[0; 2]; 2];
    let mut closer_index = 0;

    while closer_index < atoms.len() {
        let Some(closer) = delimiter_at(atoms, closer_index)
            .filter(|delimiter| delimiter.can_close && delimiter.len > 0)
        else {
            closer_index += 1;
            continue;
        };
        let bottom = delimiter_bottom(closer, &emphasis_bottom, &extension_bottom);
        let Some(opener_index) = find_compatible_opener(atoms, bottom, closer_index, closer) else {
            set_delimiter_bottom(
                closer,
                closer_index + usize::from(!closer.can_open),
                &mut emphasis_bottom,
                &mut extension_bottom,
            );
            closer_index += 1;
            continue;
        };

        let opener = delimiter_at(atoms, opener_index)
            .expect("compatible opener index points at a delimiter");
        let use_len = if closer.kind.is_emphasis() {
            if opener.len >= 2 && closer.len >= 2 {
                2
            } else {
                1
            }
        } else {
            closer.len
        };
        let opener_span = consume_opener(&mut atoms[opener_index], use_len);
        let closer_span = consume_closer(&mut atoms[closer_index], use_len);
        let children = atoms_to_inlines(atoms.drain(opener_index + 1..closer_index).collect());
        let span = Span {
            start: opener_span.start,
            end: closer_span.end,
        };
        let node = match closer.kind {
            DelimiterKind::Asterisk | DelimiterKind::Underscore if use_len == 2 => {
                Inline::Strong { span, children }
            }
            DelimiterKind::Asterisk | DelimiterKind::Underscore => {
                Inline::Emphasis { span, children }
            }
            DelimiterKind::Strikethrough => Inline::Strikethrough { span, children },
            DelimiterKind::Highlight => Inline::Highlight { span, children },
        };
        atoms.insert(opener_index + 1, Atom::Inline(node));
        repair_delimiter_bottoms(
            &mut emphasis_bottom,
            &mut extension_bottom,
            opener_index,
            closer_index,
        );
        closer_index = opener_index + 2;
        if delimiter_at(atoms, closer_index).is_none_or(|delimiter| delimiter.len == 0) {
            closer_index += 1;
        }
    }
}

#[derive(Clone, Copy)]
struct Delimiter {
    kind: DelimiterKind,
    len: u32,
    can_open: bool,
    can_close: bool,
}

impl DelimiterKind {
    fn is_emphasis(self) -> bool {
        matches!(self, Self::Asterisk | Self::Underscore)
    }
}

fn delimiter_at(atoms: &[Atom], index: usize) -> Option<Delimiter> {
    let Atom::Delimiter {
        kind,
        len,
        can_open,
        can_close,
        ..
    } = atoms.get(index)?
    else {
        return None;
    };
    Some(Delimiter {
        kind: *kind,
        len: *len,
        can_open: *can_open,
        can_close: *can_close,
    })
}

fn delimiter_bottom(
    delimiter: Delimiter,
    emphasis: &[[[usize; 3]; 2]; 2],
    extensions: &[[usize; 2]; 2],
) -> usize {
    match delimiter.kind {
        DelimiterKind::Asterisk | DelimiterKind::Underscore => {
            let kind = usize::from(delimiter.kind == DelimiterKind::Underscore);
            emphasis[kind][usize::from(delimiter.can_open)][(delimiter.len % 3) as usize]
        }
        DelimiterKind::Strikethrough => extensions[0][extension_len_index(delimiter.len)],
        DelimiterKind::Highlight => extensions[1][extension_len_index(delimiter.len)],
    }
}

fn set_delimiter_bottom(
    delimiter: Delimiter,
    value: usize,
    emphasis: &mut [[[usize; 3]; 2]; 2],
    extensions: &mut [[usize; 2]; 2],
) {
    match delimiter.kind {
        DelimiterKind::Asterisk | DelimiterKind::Underscore => {
            let kind = usize::from(delimiter.kind == DelimiterKind::Underscore);
            emphasis[kind][usize::from(delimiter.can_open)][(delimiter.len % 3) as usize] = value;
        }
        DelimiterKind::Strikethrough => {
            extensions[0][extension_len_index(delimiter.len)] = value;
        }
        DelimiterKind::Highlight => {
            extensions[1][extension_len_index(delimiter.len)] = value;
        }
    }
}

fn extension_len_index(len: u32) -> usize {
    // Participation rules cap extension runs at length 2.
    usize::from(len == 2)
}

fn find_compatible_opener(
    atoms: &[Atom],
    bottom: usize,
    closer_index: usize,
    closer: Delimiter,
) -> Option<usize> {
    for opener_index in (bottom..closer_index).rev() {
        let Some(opener) = delimiter_at(atoms, opener_index) else {
            continue;
        };
        if !opener.can_open || opener.len == 0 || opener.kind != closer.kind {
            continue;
        }
        if closer.kind.is_emphasis() {
            let rule_of_three_applies = (closer.can_open || opener.can_close)
                && (opener.len + closer.len).is_multiple_of(3)
                && !(opener.len.is_multiple_of(3) && closer.len.is_multiple_of(3));
            if !rule_of_three_applies {
                return Some(opener_index);
            }
        } else if opener.len == closer.len {
            return Some(opener_index);
        }
    }
    None
}

fn consume_opener(atom: &mut Atom, use_len: u32) -> Span {
    let Atom::Delimiter { span, len, .. } = atom else {
        unreachable!("opener is not a delimiter");
    };
    let consumed = Span {
        start: span.end - use_len,
        end: span.end,
    };
    span.end -= use_len;
    *len -= use_len;
    consumed
}

fn consume_closer(atom: &mut Atom, use_len: u32) -> Span {
    let Atom::Delimiter { span, len, .. } = atom else {
        unreachable!("closer is not a delimiter");
    };
    let consumed = Span {
        start: span.start,
        end: span.start + use_len,
    };
    span.start += use_len;
    *len -= use_len;
    consumed
}

fn repair_delimiter_bottoms(
    emphasis: &mut [[[usize; 3]; 2]; 2],
    extensions: &mut [[usize; 2]; 2],
    opener: usize,
    closer: usize,
) {
    for kind in emphasis {
        for can_open in kind {
            for bottom in can_open {
                *bottom = remap_delimiter_index(*bottom, opener, closer);
            }
        }
    }
    for kind in extensions {
        for bottom in kind {
            *bottom = remap_delimiter_index(*bottom, opener, closer);
        }
    }
}

fn remap_delimiter_index(index: usize, opener: usize, closer: usize) -> usize {
    if index <= opener {
        index
    } else if index < closer {
        opener + 1
    } else {
        opener + 2 + (index - closer)
    }
}

fn atoms_to_inlines(atoms: Vec<Atom>) -> Vec<Inline> {
    let mut inlines = Vec::new();
    for atom in atoms {
        match atom {
            Atom::Inline(inline) => push_inline(&mut inlines, inline),
            Atom::Bracket { span } => push_inline(&mut inlines, Inline::Text { span }),
            Atom::Delimiter { span, len, .. } if len > 0 => {
                push_inline(&mut inlines, Inline::Text { span })
            }
            Atom::Delimiter { .. } => {}
        }
    }
    inlines
}

fn push_text_atom(atoms: &mut Vec<Atom>, span: Span) {
    if span.start >= span.end {
        return;
    }
    if let Some(Atom::Inline(Inline::Text { span: previous })) = atoms.last_mut()
        && previous.end == span.start
    {
        previous.end = span.end;
    } else {
        atoms.push(Atom::Inline(Inline::Text { span }));
    }
}

fn push_inline(inlines: &mut Vec<Inline>, inline: Inline) {
    if let Inline::Text { span } = inline
        && let Some(Inline::Text { span: previous }) = inlines.last_mut()
        && previous.end == span.start
    {
        previous.end = span.end;
        return;
    }
    inlines.push(inline);
}

fn ascii_run_len(source: &str, start: u32, end: u32, byte: u8) -> u32 {
    source[start as usize..end as usize]
        .bytes()
        .take_while(|candidate| *candidate == byte)
        .count() as u32
}
