//! Block parser for Notes Markdown.
//!
//! This crate turns the context-free token stream from `context-lexer`
//! into a source-backed document tree. The current phase recognizes
//! frontmatter, CommonMark block structure, and core inline content represented
//! by source-backed nodes.

mod ast;
mod block;
mod diag;
mod inline;
pub(crate) mod lines;
mod span;

pub use ast::{
    Block, CodeBlockKind, Document, FrontmatterBlock, HeadingKind, Inline, LinkDefinition,
    LinkTarget, ListItem, ListKind, TableAlignment, TableCell, TableRow,
};
pub use diag::{Diagnostic, DiagnosticKind};
pub use span::Span;

/// Parses a complete Notes Markdown document.
pub fn parse(input: &str) -> Document {
    inline::finish_document(input, block::parse(input))
}

/// Decodes one complete CommonMark character reference.
///
/// The input must include the leading ampersand and trailing semicolon.
pub fn decode_character_reference(reference: &str) -> Option<String> {
    inline::decode_character_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph(span: Span, fragments: Vec<Span>) -> Block {
        Block::Paragraph {
            span,
            content: plain_content(&fragments),
        }
    }

    fn heading(span: Span, level: u8, kind: HeadingKind, fragments: Vec<Span>) -> Block {
        Block::Heading {
            span,
            level,
            kind,
            content: plain_content(&fragments),
        }
    }

    fn plain_content(fragments: &[Span]) -> Vec<Inline> {
        let mut content = Vec::new();
        for (index, fragment) in fragments.iter().enumerate() {
            if index > 0 {
                let position = fragments[index - 1].end;
                content.push(Inline::SoftBreak {
                    span: span(position, position),
                });
            }
            if fragment.start < fragment.end {
                content.push(Inline::Text { span: *fragment });
            }
        }
        content
    }

    fn code_block(span: Span, kind: CodeBlockKind, literal: Vec<Span>) -> Block {
        Block::CodeBlock {
            span,
            kind,
            literal,
        }
    }

    fn block_quote(span: Span, children: Vec<Block>) -> Block {
        Block::BlockQuote { span, children }
    }

    fn list(span: Span, kind: ListKind, tight: bool, items: Vec<ListItem>) -> Block {
        Block::List {
            span,
            kind,
            tight,
            items,
        }
    }

    fn item(span: Span, children: Vec<Block>) -> ListItem {
        ListItem {
            span,
            task: None,
            children,
        }
    }

    fn span(start: u32, end: u32) -> Span {
        Span { start, end }
    }

    fn empty_document() -> Document {
        Document {
            frontmatter: None,
            children: Vec::new(),
            definitions: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn empty_input_produces_an_empty_document() {
        assert_eq!(parse(""), empty_document());
    }

    #[test]
    fn whitespace_and_blank_only_input_has_no_children() {
        assert_eq!(parse("  \n\t\r\n\n"), empty_document());
    }

    #[test]
    fn single_line_paragraph_has_exact_span_and_fragment() {
        assert_eq!(
            parse("hello"),
            Document {
                frontmatter: None,
                children: vec![paragraph(
                    Span { start: 0, end: 5 },
                    vec![Span { start: 0, end: 5 }],
                )],
                definitions: Vec::new(),
                diagnostics: Vec::new(),
            }
        );
    }

    #[test]
    fn consecutive_non_blank_lines_form_one_paragraph() {
        assert_eq!(
            parse("one\ntwo\nthree"),
            Document {
                frontmatter: None,
                children: vec![paragraph(
                    Span { start: 0, end: 13 },
                    vec![
                        Span { start: 0, end: 3 },
                        Span { start: 4, end: 7 },
                        Span { start: 8, end: 13 },
                    ],
                )],
                definitions: Vec::new(),
                diagnostics: Vec::new(),
            }
        );
    }

    #[test]
    fn one_blank_line_separates_paragraphs() {
        assert_eq!(
            parse("one\n\ntwo"),
            Document {
                frontmatter: None,
                children: vec![
                    paragraph(Span { start: 0, end: 3 }, vec![Span { start: 0, end: 3 }],),
                    paragraph(Span { start: 5, end: 8 }, vec![Span { start: 5, end: 8 }],),
                ],
                definitions: Vec::new(),
                diagnostics: Vec::new(),
            }
        );
    }

    #[test]
    fn multiple_blank_lines_separate_paragraphs() {
        assert_eq!(
            parse("one\n\n \t\n\ntwo").children,
            vec![
                paragraph(Span { start: 0, end: 3 }, vec![Span { start: 0, end: 3 }],),
                paragraph(Span { start: 9, end: 12 }, vec![Span { start: 9, end: 12 }],),
            ]
        );
    }

    #[test]
    fn leading_indentation_and_final_trailing_spaces_are_excluded_from_inline_text() {
        assert_eq!(
            parse("   text  ").children,
            vec![Block::Paragraph {
                span: Span { start: 3, end: 9 },
                content: vec![Inline::Text {
                    span: Span { start: 3, end: 7 },
                }],
            }]
        );
    }

    #[test]
    fn crlf_is_excluded_from_fragments_without_losing_absolute_offsets() {
        assert_eq!(
            parse("one\r\ntwo\r\n").children,
            vec![paragraph(
                Span { start: 0, end: 8 },
                vec![Span { start: 0, end: 3 }, Span { start: 5, end: 8 }],
            )]
        );
    }

    #[test]
    fn invalid_block_marker_runs_remain_paragraph_text() {
        assert_eq!(
            parse("####### x\n-*-\n--x").children,
            vec![paragraph(
                span(0, 17),
                vec![span(0, 9), span(10, 13), span(14, 17)],
            )]
        );
    }

    #[test]
    fn atx_heading_recognizes_levels_one_through_six() {
        for level in 1..=6 {
            let input = format!("{} heading", "#".repeat(level));
            let content_start = level as u32 + 1;
            assert_eq!(
                parse(&input).children,
                vec![heading(
                    span(0, input.len() as u32),
                    level as u8,
                    HeadingKind::Atx,
                    vec![span(content_start, input.len() as u32)],
                )],
                "level = {level}"
            );
        }
    }

    #[test]
    fn invalid_atx_markers_are_paragraphs() {
        assert_eq!(
            parse("####### x\n#foo").children,
            vec![paragraph(span(0, 14), vec![span(0, 9), span(10, 14)])]
        );
    }

    #[test]
    fn empty_atx_heading_has_no_fragments() {
        assert_eq!(
            parse("#").children,
            vec![heading(span(0, 1), 1, HeadingKind::Atx, Vec::new())]
        );
        assert_eq!(
            parse("## ##").children,
            vec![heading(span(0, 5), 2, HeadingKind::Atx, Vec::new())]
        );
    }

    #[test]
    fn atx_heading_accepts_three_columns_but_four_columns_are_indented_code() {
        assert_eq!(
            parse("   ### x").children,
            vec![heading(span(3, 8), 3, HeadingKind::Atx, vec![span(7, 8)],)]
        );
        assert_eq!(
            parse("    ### x").children,
            vec![code_block(
                span(4, 9),
                CodeBlockKind::Indented,
                vec![span(4, 9)],
            )]
        );
    }

    #[test]
    fn atx_decorative_closer_is_stripped_only_when_preceded_by_whitespace() {
        assert_eq!(
            parse("## foo ##").children,
            vec![heading(span(0, 9), 2, HeadingKind::Atx, vec![span(3, 6)],)]
        );
        assert_eq!(
            parse("# foo#").children,
            vec![heading(span(0, 6), 1, HeadingKind::Atx, vec![span(2, 6)],)]
        );
    }

    #[test]
    fn atx_heading_interrupts_an_open_paragraph() {
        assert_eq!(
            parse("para\n# head").children,
            vec![
                paragraph(span(0, 4), vec![span(0, 4)]),
                heading(span(5, 11), 1, HeadingKind::Atx, vec![span(7, 11)]),
            ]
        );
    }

    #[test]
    fn thematic_break_accepts_each_marker_and_internal_or_trailing_whitespace() {
        let cases = [
            ("\n---", span(1, 4)),
            ("***", span(0, 3)),
            ("___", span(0, 3)),
            ("- - -", span(0, 5)),
            ("***  ", span(0, 5)),
        ];
        for (input, expected_span) in cases {
            assert_eq!(
                parse(input).children,
                vec![Block::ThematicBreak {
                    span: expected_span
                }],
                "input = {input:?}"
            );
        }
    }

    #[test]
    fn thematic_break_rejects_too_few_or_mixed_markers() {
        assert_eq!(
            parse("--\n-*-").children,
            vec![paragraph(span(0, 6), vec![span(0, 2), span(3, 6)])]
        );
    }

    #[test]
    fn setext_underlines_convert_single_and_multiline_paragraphs() {
        assert_eq!(
            parse("title\n===").children,
            vec![heading(
                span(0, 9),
                1,
                HeadingKind::Setext,
                vec![span(0, 5)],
            )]
        );
        assert_eq!(
            parse("one\ntwo\n---").children,
            vec![heading(
                span(0, 11),
                2,
                HeadingKind::Setext,
                vec![span(0, 3), span(4, 7)],
            )]
        );
    }

    #[test]
    fn setext_headings_extract_leading_link_definitions() {
        let document = parse("[foo]: /url\nbar\n===\n\n[foo]");
        assert_eq!(
            document.definitions,
            vec![LinkDefinition {
                span: span(0, 11),
                label: span(1, 4),
                dest: span(7, 11),
                title: None,
            }]
        );
        assert_eq!(
            document.children,
            vec![
                Block::Heading {
                    span: span(12, 19),
                    level: 1,
                    kind: HeadingKind::Setext,
                    content: vec![Inline::Text { span: span(12, 15) }],
                },
                Block::Paragraph {
                    span: span(21, 26),
                    content: vec![Inline::Link {
                        span: span(21, 26),
                        target: LinkTarget {
                            dest: Some(span(7, 11)),
                            title: None,
                            label: Some(span(22, 25)),
                        },
                        children: vec![Inline::Text { span: span(22, 25) }],
                    }],
                },
            ]
        );
    }

    #[test]
    fn contentless_setext_equals_underline_becomes_a_paragraph() {
        let document = parse("[foo]: /url\n===");

        assert_eq!(
            document.definitions,
            vec![LinkDefinition {
                span: span(0, 11),
                label: span(1, 4),
                dest: span(7, 11),
                title: None,
            }]
        );
        assert_eq!(
            document.children,
            vec![paragraph(span(12, 15), vec![span(12, 15)])]
        );

        let indented = parse("[foo]: /url\n   ===  ");
        assert_eq!(
            indented.children,
            vec![Block::Paragraph {
                span: span(15, 20),
                content: vec![Inline::Text { span: span(15, 18) }],
            }]
        );
    }

    #[test]
    fn contentless_setext_equals_underline_merges_with_the_next_paragraph() {
        let document = parse("[foo]: /url\n===\n[foo]");

        assert_eq!(document.definitions.len(), 1);
        assert_eq!(
            document.children,
            vec![Block::Paragraph {
                span: span(12, 21),
                content: vec![
                    Inline::Text { span: span(12, 15) },
                    Inline::SoftBreak { span: span(15, 15) },
                    Inline::Link {
                        span: span(16, 21),
                        target: LinkTarget {
                            dest: Some(span(7, 11)),
                            title: None,
                            label: Some(span(17, 20)),
                        },
                        children: vec![Inline::Text { span: span(17, 20) }],
                    },
                ],
            }]
        );

        let crlf = parse("[foo]: /url\r\n===\r\n[foo]");
        assert!(matches!(
            &crlf.children[..],
            [Block::Paragraph {
                span: Span { start: 13, end: 23 },
                content,
            }] if matches!(
                &content[..],
                [Inline::Text { span: Span { start: 13, end: 16 } },
                 Inline::SoftBreak { span: Span { start: 16, end: 16 } },
                 Inline::Link { span: Span { start: 18, end: 23 }, .. }]
            )
        ));
    }

    #[test]
    fn contentless_setext_merge_precedes_definition_extraction() {
        let input = "[foo]: /url\n===\n[bar]: /baz";
        let document = parse(input);

        assert_eq!(document.definitions.len(), 1);
        assert_eq!(document.definitions[0].label.slice(input), "foo");
        assert_eq!(
            document.children,
            vec![paragraph(span(12, 27), vec![span(12, 15), span(16, 27)],)]
        );
    }

    #[test]
    fn contentless_setext_equals_underline_does_not_merge_across_a_blank_line() {
        let document = parse("[foo]: /url\n===\n\nnext");

        assert_eq!(document.definitions.len(), 1);
        assert_eq!(
            document.children,
            vec![
                paragraph(span(12, 15), vec![span(12, 15)]),
                paragraph(span(17, 21), vec![span(17, 21)]),
            ]
        );
    }

    #[test]
    fn contentless_setext_dash_underline_becomes_a_thematic_break() {
        let document = parse("[foo]: /url\n---");

        assert_eq!(document.definitions.len(), 1);
        assert_eq!(
            document.children,
            vec![Block::ThematicBreak { span: span(12, 15) }]
        );
    }

    #[test]
    fn contentless_setext_reconstruction_recurses_into_blockquotes() {
        let equals = parse("> [foo]: /url\n> ===");
        assert_eq!(equals.definitions.len(), 1);
        assert_eq!(
            equals.children,
            vec![block_quote(
                span(0, 19),
                vec![paragraph(span(16, 19), vec![span(16, 19)])],
            )]
        );

        let merged = parse("> [foo]: /url\n> ===\n> [foo]");
        assert_eq!(merged.definitions.len(), 1);
        assert_eq!(
            merged.children,
            vec![block_quote(
                span(0, 27),
                vec![Block::Paragraph {
                    span: span(16, 27),
                    content: vec![
                        Inline::Text { span: span(16, 19) },
                        Inline::SoftBreak { span: span(19, 19) },
                        Inline::Link {
                            span: span(22, 27),
                            target: LinkTarget {
                                dest: Some(span(9, 13)),
                                title: None,
                                label: Some(span(23, 26)),
                            },
                            children: vec![Inline::Text { span: span(23, 26) }],
                        },
                    ],
                }],
            )]
        );

        let dash = parse("> [foo]: /url\n> ---");
        assert_eq!(dash.definitions.len(), 1);
        assert_eq!(
            dash.children,
            vec![block_quote(
                span(0, 19),
                vec![Block::ThematicBreak { span: span(16, 19) }],
            )]
        );
    }

    #[test]
    fn contentless_setext_reconstruction_recurses_into_list_items() {
        let merged = parse("- [foo]: /url\n  ===\n  [foo]");
        assert_eq!(merged.definitions.len(), 1);
        assert_eq!(
            merged.children,
            vec![list(
                span(0, 27),
                ListKind::Bullet { marker: b'-' },
                true,
                vec![item(
                    span(0, 27),
                    vec![Block::Paragraph {
                        span: span(16, 27),
                        content: vec![
                            Inline::Text { span: span(16, 19) },
                            Inline::SoftBreak { span: span(19, 19) },
                            Inline::Link {
                                span: span(22, 27),
                                target: LinkTarget {
                                    dest: Some(span(9, 13)),
                                    title: None,
                                    label: Some(span(23, 26)),
                                },
                                children: vec![Inline::Text { span: span(23, 26) }],
                            },
                        ],
                    }],
                )],
            )]
        );

        let dash = parse("- [foo]: /url\n  ---");
        assert_eq!(dash.definitions.len(), 1);
        assert_eq!(
            dash.children,
            vec![list(
                span(0, 19),
                ListKind::Bullet { marker: b'-' },
                true,
                vec![item(
                    span(0, 19),
                    vec![Block::ThematicBreak { span: span(16, 19) }],
                )],
            )]
        );
    }

    #[test]
    fn multiline_definition_can_leave_a_contentless_setext_underline() {
        let input = "[foo]:\n/url\n\"title\"\n===\n[foo]";
        let document = parse(input);

        assert_eq!(document.definitions.len(), 1);
        assert_eq!(document.definitions[0].span, span(0, 19));
        assert_eq!(document.definitions[0].dest, span(7, 11));
        assert_eq!(document.definitions[0].title, Some(span(13, 18)));
        assert_eq!(
            document.children,
            vec![Block::Paragraph {
                span: span(20, 29),
                content: vec![
                    Inline::Text { span: span(20, 23) },
                    Inline::SoftBreak { span: span(23, 23) },
                    Inline::Link {
                        span: span(24, 29),
                        target: LinkTarget {
                            dest: Some(span(7, 11)),
                            title: Some(span(13, 18)),
                            label: Some(span(25, 28)),
                        },
                        children: vec![Inline::Text { span: span(25, 28) }],
                    },
                ],
            }]
        );
    }

    #[test]
    fn setext_takes_precedence_over_thematic_break_after_a_paragraph() {
        assert_eq!(
            parse("title\n---").children,
            vec![heading(
                span(0, 9),
                2,
                HeadingKind::Setext,
                vec![span(0, 5)],
            )]
        );
    }

    #[test]
    fn setext_requires_an_open_paragraph_and_a_single_run() {
        assert_eq!(
            parse("===").children,
            vec![paragraph(span(0, 3), vec![span(0, 3)])]
        );
        assert_eq!(
            parse("title\n= =").children,
            vec![paragraph(span(0, 9), vec![span(0, 5), span(6, 9)])]
        );
    }

    #[test]
    fn fenced_code_records_trimmed_info_and_none_when_empty() {
        assert_eq!(
            parse("``` rust  \n# x\n```\n").children,
            vec![code_block(
                span(0, 18),
                CodeBlockKind::Fenced {
                    info: Some(span(4, 8)),
                },
                vec![span(11, 14)],
            )]
        );
        assert_eq!(
            parse("```\n```").children,
            vec![code_block(
                span(0, 7),
                CodeBlockKind::Fenced { info: None },
                Vec::new(),
            )]
        );
    }

    #[test]
    fn tilde_fence_keeps_block_markers_as_verbatim_content() {
        assert_eq!(
            parse("~~~ lang\n# x\n---\n~~~").children,
            vec![code_block(
                span(0, 20),
                CodeBlockKind::Fenced {
                    info: Some(span(4, 8)),
                },
                vec![span(9, 12), span(13, 16)],
            )]
        );
    }

    #[test]
    fn fence_closer_must_match_marker_and_be_at_least_as_long() {
        assert_eq!(
            parse("````\n```\n`````\n").children,
            vec![code_block(
                span(0, 14),
                CodeBlockKind::Fenced { info: None },
                vec![span(5, 8)],
            )]
        );
    }

    #[test]
    fn fenced_code_keeps_blank_content_lines_as_empty_spans() {
        assert_eq!(
            parse("```\n\nx\n```").children,
            vec![code_block(
                span(0, 10),
                CodeBlockKind::Fenced { info: None },
                vec![span(4, 4), span(5, 6)],
            )]
        );
    }

    #[test]
    fn unclosed_fence_reaches_eof_and_emits_a_diagnostic() {
        assert_eq!(
            parse("```\nbody"),
            Document {
                frontmatter: None,
                children: vec![code_block(
                    span(0, 8),
                    CodeBlockKind::Fenced { info: None },
                    vec![span(4, 8)],
                )],
                definitions: Vec::new(),
                diagnostics: vec![Diagnostic {
                    span: span(0, 3),
                    kind: DiagnosticKind::UnclosedCodeFence,
                }],
            }
        );
    }

    #[test]
    fn backtick_in_backtick_fence_info_rejects_the_fence() {
        assert_eq!(
            parse("``` bad`info\ntext").children,
            vec![paragraph(span(0, 17), vec![span(0, 12), span(13, 17)])]
        );
    }

    #[test]
    fn fenced_code_strips_up_to_the_opening_indent_from_content() {
        assert_eq!(
            parse("  ```\n  one\n    two\n  ```").children,
            vec![code_block(
                span(2, 25),
                CodeBlockKind::Fenced { info: None },
                vec![span(8, 11), span(14, 19)],
            )]
        );
    }

    #[test]
    fn indented_code_strips_four_columns_and_tabs_count_as_four() {
        assert_eq!(
            parse("    code").children,
            vec![code_block(
                span(4, 8),
                CodeBlockKind::Indented,
                vec![span(4, 8)],
            )]
        );
        assert_eq!(
            parse("\tcode").children,
            vec![code_block(
                span(1, 5),
                CodeBlockKind::Indented,
                vec![span(1, 5)],
            )]
        );
    }

    #[test]
    fn indented_line_after_paragraph_is_lazy_continuation() {
        assert_eq!(
            parse("text\n    code").children,
            vec![paragraph(span(0, 13), vec![span(0, 4), span(9, 13)])]
        );
    }

    #[test]
    fn indented_code_keeps_interior_but_drops_trailing_blank_lines() {
        assert_eq!(
            parse("    one\n\n    two\n\n").children,
            vec![code_block(
                span(4, 16),
                CodeBlockKind::Indented,
                vec![span(4, 7), span(8, 8), span(13, 16)],
            )]
        );
    }

    #[test]
    fn fenced_code_preserves_whitespace_only_content_lines() {
        assert_eq!(
            parse("```\n   \nx\n```").children,
            vec![code_block(
                span(0, 13),
                CodeBlockKind::Fenced { info: None },
                vec![span(4, 7), span(8, 9)],
            )]
        );
    }

    #[test]
    fn indented_code_preserves_whitespace_remaining_after_four_columns() {
        assert_eq!(
            parse("    one\n      \n    two").children,
            vec![code_block(
                span(4, 22),
                CodeBlockKind::Indented,
                vec![span(4, 7), span(12, 14), span(19, 22)],
            )]
        );
    }

    #[test]
    fn blockquote_single_and_multiline_paragraphs_exclude_markers() {
        assert_eq!(
            parse("> a").children,
            vec![block_quote(
                span(0, 3),
                vec![paragraph(span(2, 3), vec![span(2, 3)])],
            )]
        );
        assert_eq!(
            parse("> a\n> b").children,
            vec![block_quote(
                span(0, 7),
                vec![paragraph(span(2, 7), vec![span(2, 3), span(6, 7)])],
            )]
        );
    }

    #[test]
    fn blockquote_allows_lazy_paragraph_continuation() {
        assert_eq!(
            parse("> a\nb").children,
            vec![block_quote(
                span(0, 5),
                vec![paragraph(span(2, 5), vec![span(2, 3), span(4, 5)])],
            )]
        );
    }

    #[test]
    fn blank_line_closes_blockquote_but_marked_blank_splits_inner_paragraphs() {
        assert_eq!(
            parse("> a\n\nb").children,
            vec![
                block_quote(span(0, 3), vec![paragraph(span(2, 3), vec![span(2, 3)])],),
                paragraph(span(5, 6), vec![span(5, 6)]),
            ]
        );
        assert_eq!(
            parse("> a\n>\n> b").children,
            vec![block_quote(
                span(0, 9),
                vec![
                    paragraph(span(2, 3), vec![span(2, 3)]),
                    paragraph(span(8, 9), vec![span(8, 9)]),
                ],
            )]
        );
    }

    #[test]
    fn blockquote_marker_needs_no_following_space() {
        assert_eq!(
            parse(">quote").children,
            vec![block_quote(
                span(0, 6),
                vec![paragraph(span(1, 6), vec![span(1, 6)])],
            )]
        );
    }

    #[test]
    fn adjacent_and_spaced_gt_markers_create_the_same_nesting_depth() {
        let adjacent = parse(">> a");
        let spaced = parse("> > a");
        assert_eq!(blockquote_depth(&adjacent.children[0]), 2);
        assert_eq!(blockquote_depth(&spaced.children[0]), 2);
    }

    #[test]
    fn blockquote_can_contain_heading_and_unclosed_fence() {
        assert_eq!(
            parse("> # h").children,
            vec![block_quote(
                span(0, 5),
                vec![heading(span(2, 5), 1, HeadingKind::Atx, vec![span(4, 5)],)],
            )]
        );

        let parsed = parse("> ```\n> x\nout");
        assert_eq!(
            parsed.children,
            vec![
                block_quote(
                    span(0, 9),
                    vec![code_block(
                        span(2, 9),
                        CodeBlockKind::Fenced { info: None },
                        vec![span(8, 9)],
                    )],
                ),
                paragraph(span(10, 13), vec![span(10, 13)]),
            ]
        );
        assert_eq!(
            parsed.diagnostics,
            vec![Diagnostic {
                span: span(2, 5),
                kind: DiagnosticKind::UnclosedCodeFence,
            }]
        );
    }

    #[test]
    fn tight_bullet_list_groups_three_items() {
        assert_eq!(
            parse("- a\n- b\n- c").children,
            vec![list(
                span(0, 11),
                ListKind::Bullet { marker: b'-' },
                true,
                vec![
                    item(span(0, 3), vec![paragraph(span(2, 3), vec![span(2, 3)])]),
                    item(span(4, 7), vec![paragraph(span(6, 7), vec![span(6, 7)])]),
                    item(
                        span(8, 11),
                        vec![paragraph(span(10, 11), vec![span(10, 11)])],
                    ),
                ],
            )]
        );
    }

    #[test]
    fn star_and_plus_bullets_are_recognized_and_marker_changes_split_lists() {
        assert_eq!(
            parse("* a").children,
            vec![list(
                span(0, 3),
                ListKind::Bullet { marker: b'*' },
                true,
                vec![item(
                    span(0, 3),
                    vec![paragraph(span(2, 3), vec![span(2, 3)])],
                )],
            )]
        );
        assert_eq!(
            parse("+ a").children,
            vec![list(
                span(0, 3),
                ListKind::Bullet { marker: b'+' },
                true,
                vec![item(
                    span(0, 3),
                    vec![paragraph(span(2, 3), vec![span(2, 3)])],
                )],
            )]
        );
        assert_eq!(
            parse("- a\n* b").children,
            vec![
                list(
                    span(0, 3),
                    ListKind::Bullet { marker: b'-' },
                    true,
                    vec![item(
                        span(0, 3),
                        vec![paragraph(span(2, 3), vec![span(2, 3)])],
                    )],
                ),
                list(
                    span(4, 7),
                    ListKind::Bullet { marker: b'*' },
                    true,
                    vec![item(
                        span(4, 7),
                        vec![paragraph(span(6, 7), vec![span(6, 7)])],
                    )],
                ),
            ]
        );
    }

    #[test]
    fn ordered_list_records_start_and_delimiter_and_rejects_glued_text() {
        assert_eq!(
            parse("3) a\n4) b").children,
            vec![list(
                span(0, 9),
                ListKind::Ordered {
                    start: 3,
                    delimiter: b')',
                },
                true,
                vec![
                    item(span(0, 4), vec![paragraph(span(3, 4), vec![span(3, 4)])]),
                    item(span(5, 9), vec![paragraph(span(8, 9), vec![span(8, 9)])]),
                ],
            )]
        );
        assert_eq!(
            parse("1.x").children,
            vec![paragraph(span(0, 3), vec![span(0, 3)])]
        );
        assert_eq!(
            parse("1. a\n2) b").children,
            vec![
                list(
                    span(0, 4),
                    ListKind::Ordered {
                        start: 1,
                        delimiter: b'.',
                    },
                    true,
                    vec![item(
                        span(0, 4),
                        vec![paragraph(span(3, 4), vec![span(3, 4)])],
                    )],
                ),
                list(
                    span(5, 9),
                    ListKind::Ordered {
                        start: 2,
                        delimiter: b')',
                    },
                    true,
                    vec![item(
                        span(5, 9),
                        vec![paragraph(span(8, 9), vec![span(8, 9)])],
                    )],
                ),
            ]
        );
    }

    #[test]
    fn list_can_nest_at_the_parent_item_content_column() {
        assert_eq!(
            parse("- a\n  - b").children,
            vec![list(
                span(0, 9),
                ListKind::Bullet { marker: b'-' },
                true,
                vec![item(
                    span(0, 9),
                    vec![
                        paragraph(span(2, 3), vec![span(2, 3)]),
                        list(
                            span(6, 9),
                            ListKind::Bullet { marker: b'-' },
                            true,
                            vec![item(
                                span(6, 9),
                                vec![paragraph(span(8, 9), vec![span(8, 9)])],
                            )],
                        ),
                    ],
                )],
            )]
        );
    }

    #[test]
    fn blank_lines_between_items_or_blocks_make_lists_loose() {
        assert_eq!(
            parse("- a\n\n- b").children,
            vec![list(
                span(0, 8),
                ListKind::Bullet { marker: b'-' },
                false,
                vec![
                    item(span(0, 3), vec![paragraph(span(2, 3), vec![span(2, 3)])]),
                    item(span(5, 8), vec![paragraph(span(7, 8), vec![span(7, 8)])]),
                ],
            )]
        );
        assert_eq!(
            parse("- a\n\n  b").children,
            vec![list(
                span(0, 8),
                ListKind::Bullet { marker: b'-' },
                false,
                vec![item(
                    span(0, 8),
                    vec![
                        paragraph(span(2, 3), vec![span(2, 3)]),
                        paragraph(span(7, 8), vec![span(7, 8)]),
                    ],
                )],
            )]
        );
    }

    #[test]
    fn five_spaces_after_marker_create_indented_code_at_marker_plus_one() {
        assert_eq!(
            parse("-     code").children,
            vec![list(
                span(0, 10),
                ListKind::Bullet { marker: b'-' },
                true,
                vec![item(
                    span(0, 10),
                    vec![code_block(
                        span(6, 10),
                        CodeBlockKind::Indented,
                        vec![span(6, 10)],
                    )],
                )],
            )]
        );
    }

    #[test]
    fn empty_middle_item_stays_in_the_list() {
        assert_eq!(
            parse("- a\n-\n- c").children,
            vec![list(
                span(0, 9),
                ListKind::Bullet { marker: b'-' },
                true,
                vec![
                    item(span(0, 3), vec![paragraph(span(2, 3), vec![span(2, 3)])]),
                    item(span(4, 5), Vec::new()),
                    item(span(6, 9), vec![paragraph(span(8, 9), vec![span(8, 9)])]),
                ],
            )]
        );
    }

    #[test]
    fn empty_item_continuation_distinguishes_content_from_a_real_blank_line() {
        assert_eq!(
            parse("-\n  foo").children,
            vec![list(
                span(0, 7),
                ListKind::Bullet { marker: b'-' },
                true,
                vec![item(
                    span(0, 7),
                    vec![Block::Paragraph {
                        span: span(4, 7),
                        content: vec![Inline::Text { span: span(4, 7) }],
                    }],
                )],
            )]
        );
        assert_eq!(
            parse("-\n\n  foo").children,
            vec![
                list(
                    span(0, 1),
                    ListKind::Bullet { marker: b'-' },
                    true,
                    vec![item(span(0, 1), Vec::new())],
                ),
                Block::Paragraph {
                    span: span(5, 8),
                    content: vec![Inline::Text { span: span(5, 8) }],
                },
            ]
        );
        assert_eq!(
            parse("-\n\n- b").children,
            vec![list(
                span(0, 6),
                ListKind::Bullet { marker: b'-' },
                true,
                vec![
                    item(span(0, 1), Vec::new()),
                    item(
                        span(3, 6),
                        vec![Block::Paragraph {
                            span: span(5, 6),
                            content: vec![Inline::Text { span: span(5, 6) }],
                        }],
                    ),
                ],
            )]
        );
    }

    #[test]
    fn list_interruption_rules_and_setext_precedence_are_respected() {
        assert_eq!(
            parse("foo\n- bar").children,
            vec![
                paragraph(span(0, 3), vec![span(0, 3)]),
                list(
                    span(4, 9),
                    ListKind::Bullet { marker: b'-' },
                    true,
                    vec![item(
                        span(4, 9),
                        vec![paragraph(span(6, 9), vec![span(6, 9)])],
                    )],
                ),
            ]
        );
        assert_eq!(
            parse("foo\n2. bar").children,
            vec![paragraph(span(0, 10), vec![span(0, 3), span(4, 10)])]
        );
        assert_eq!(
            parse("foo\n1. bar").children,
            vec![
                paragraph(span(0, 3), vec![span(0, 3)]),
                list(
                    span(4, 10),
                    ListKind::Ordered {
                        start: 1,
                        delimiter: b'.',
                    },
                    true,
                    vec![item(
                        span(4, 10),
                        vec![paragraph(span(7, 10), vec![span(7, 10)])],
                    )],
                ),
            ]
        );
        assert_eq!(
            parse("foo\n-").children,
            vec![heading(
                span(0, 5),
                2,
                HeadingKind::Setext,
                vec![span(0, 3)],
            )]
        );
        assert_eq!(
            parse("foo\n+").children,
            vec![paragraph(span(0, 5), vec![span(0, 3), span(4, 5)])]
        );
    }

    #[test]
    fn list_item_paragraph_allows_lazy_continuation() {
        assert_eq!(
            parse("- a\nb").children,
            vec![list(
                span(0, 5),
                ListKind::Bullet { marker: b'-' },
                true,
                vec![item(
                    span(0, 5),
                    vec![paragraph(span(2, 5), vec![span(2, 3), span(4, 5)])],
                )],
            )]
        );
    }

    #[test]
    fn task_list_markers_are_stripped_before_inline_parsing() {
        let document = parse("- [ ] a\n- [x] b\n- [X] c");
        let Block::List { items, .. } = &document.children[0] else {
            panic!("expected task list");
        };
        assert_eq!(
            items.iter().map(|item| item.task).collect::<Vec<_>>(),
            vec![Some(false), Some(true), Some(true)]
        );
        let expected = [(span(6, 7), "a"), (span(14, 15), "b"), (span(22, 23), "c")];
        for (item, (content_span, text)) in items.iter().zip(expected) {
            assert_eq!(
                item.children,
                vec![Block::Paragraph {
                    span: content_span,
                    content: vec![Inline::Text { span: content_span }],
                }],
                "task content = {text:?}"
            );
        }

        let with_definition = parse("- [x] a\n\n[x]: /url");
        let Block::List { items, .. } = &with_definition.children[0] else {
            panic!("expected task list");
        };
        assert_eq!(items[0].task, Some(true));
        assert!(matches!(
            &items[0].children[0],
            Block::Paragraph { content, .. }
                if matches!(&content[..], [Inline::Text { span: Span { start: 6, end: 7 } }])
        ));
    }

    #[test]
    fn task_markers_require_first_position_and_following_whitespace() {
        let document = parse("- [x](url)\n- a [x] b");
        let Block::List { items, .. } = &document.children[0] else {
            panic!("expected list");
        };
        assert_eq!(items[0].task, None);
        assert_eq!(items[1].task, None);
        assert!(matches!(
            &items[0].children[0],
            Block::Paragraph { content, .. }
                if matches!(&content[..], [Inline::Link { .. }])
        ));

        let nested = parse("- a\n  - [x] b");
        let Block::List { items, .. } = &nested.children[0] else {
            panic!("expected outer list");
        };
        let Block::List {
            items: nested_items,
            ..
        } = &items[0].children[1]
        else {
            panic!("expected nested list");
        };
        assert_eq!(nested_items[0].task, Some(true));

        let marker_only = parse("- [x]");
        let Block::List { items, .. } = &marker_only.children[0] else {
            panic!("expected marker-only task list");
        };
        assert_eq!(items[0].task, Some(true));
        assert_eq!(
            items[0].children,
            vec![Block::Paragraph {
                span: span(5, 5),
                content: Vec::new(),
            }]
        );

        let later_paragraph = parse("- first\n\n  [x] second");
        let Block::List { items, .. } = &later_paragraph.children[0] else {
            panic!("expected loose list");
        };
        assert_eq!(items[0].task, None);
    }

    #[test]
    fn blockquote_can_contain_a_list() {
        assert_eq!(
            parse("> - a\n> - b").children,
            vec![block_quote(
                span(0, 11),
                vec![list(
                    span(2, 11),
                    ListKind::Bullet { marker: b'-' },
                    true,
                    vec![
                        item(span(2, 5), vec![paragraph(span(4, 5), vec![span(4, 5)])]),
                        item(
                            span(8, 11),
                            vec![paragraph(span(10, 11), vec![span(10, 11)])],
                        ),
                    ],
                )],
            )]
        );
    }

    fn blockquote_depth(block: &Block) -> usize {
        match block {
            Block::BlockQuote { children, .. } => 1 + children.first().map_or(0, blockquote_depth),
            _ => 0,
        }
    }

    #[test]
    fn setext_underline_cannot_be_a_lazy_continuation_line() {
        // CommonMark: the underline becomes plain paragraph continuation text
        // inside the quote; it must not close the blockquote. The analogous
        // `---` IS a thematic break and does interrupt (control case).
        assert_eq!(
            parse("> foo\n===").children,
            vec![block_quote(
                span(0, 9),
                vec![paragraph(span(2, 9), vec![span(2, 5), span(6, 9)])],
            )]
        );
        assert_eq!(
            parse("> foo\n---").children,
            vec![
                block_quote(span(0, 5), vec![paragraph(span(2, 5), vec![span(2, 5)])]),
                Block::ThematicBreak { span: span(6, 9) },
            ]
        );
    }

    #[test]
    fn trailing_blank_lines_do_not_extend_item_or_list_spans() {
        assert_eq!(
            parse("- a\n\n").children,
            vec![list(
                span(0, 3),
                ListKind::Bullet { marker: b'-' },
                true,
                vec![item(
                    span(0, 3),
                    vec![paragraph(span(2, 3), vec![span(2, 3)])]
                )],
            )]
        );
    }

    #[test]
    fn escaped_backslash_at_line_end_is_not_a_hard_break() {
        // `a\\` + newline: the second backslash is escaped (literal), so the
        // line ends with no live backslash — soft break, not hard break.
        assert_eq!(
            parse("a\\\\\nb").children,
            vec![Block::Paragraph {
                span: span(0, 5),
                content: vec![
                    Inline::Text { span: span(0, 1) },
                    Inline::Escaped { span: span(1, 3) },
                    Inline::SoftBreak { span: span(3, 3) },
                    Inline::Text { span: span(4, 5) },
                ],
            }]
        );
        // Odd count keeps the hard break.
        assert_eq!(
            parse("a\\\\\\\nb").children,
            vec![Block::Paragraph {
                span: span(0, 6),
                content: vec![
                    Inline::Text { span: span(0, 1) },
                    Inline::Escaped { span: span(1, 3) },
                    Inline::HardBreak { span: span(3, 4) },
                    Inline::Text { span: span(5, 6) },
                ],
            }]
        );
    }

    #[test]
    fn overlong_numeric_character_references_stay_literal_text() {
        // Spec bounds: up to 7 decimal digits / 6 hex digits. Beyond that the
        // sequence is not a character reference at all.
        assert_eq!(
            parse("&#1234567890;").children,
            vec![Block::Paragraph {
                span: span(0, 13),
                content: vec![Inline::Text { span: span(0, 13) }],
            }]
        );
        assert_eq!(
            parse("&#x1234567;").children,
            vec![Block::Paragraph {
                span: span(0, 11),
                content: vec![Inline::Text { span: span(0, 11) }],
            }]
        );
        // The boundary itself still decodes (0x10FFFF is the maximum scalar).
        assert_eq!(
            parse("&#x10FFFF;").children,
            vec![Block::Paragraph {
                span: span(0, 10),
                content: vec![Inline::CharacterReference {
                    span: span(0, 10),
                    value: "\u{10ffff}".to_owned(),
                }],
            }]
        );
    }

    #[test]
    fn backslash_escapes_only_ascii_punctuation() {
        assert_eq!(
            parse("\\*not\\*").children,
            vec![Block::Paragraph {
                span: span(0, 7),
                content: vec![
                    Inline::Escaped { span: span(0, 2) },
                    Inline::Text { span: span(2, 5) },
                    Inline::Escaped { span: span(5, 7) },
                ],
            }]
        );
        assert_eq!(
            parse("\\a").children,
            vec![Block::Paragraph {
                span: span(0, 2),
                content: vec![Inline::Text { span: span(0, 2) }],
            }]
        );
        assert_eq!(
            parse("\\\\*em*").children,
            vec![Block::Paragraph {
                span: span(0, 6),
                content: vec![
                    Inline::Escaped { span: span(0, 2) },
                    Inline::Emphasis {
                        span: span(2, 6),
                        children: vec![Inline::Text { span: span(3, 5) }],
                    },
                ],
            }]
        );
    }

    #[test]
    fn named_and_numeric_character_references_are_decoded() {
        let cases = [
            ("&amp;", "&"),
            ("&#35;", "#"),
            ("&#x22;", "\""),
            ("&#0;", "\u{fffd}"),
            ("&#x110000;", "\u{fffd}"),
            ("&copy;", "\u{00a9}"),
            ("&fjlig;", "fj"),
        ];
        for (input, value) in cases {
            assert_eq!(
                parse(input).children,
                vec![Block::Paragraph {
                    span: span(0, input.len() as u32),
                    content: vec![Inline::CharacterReference {
                        span: span(0, input.len() as u32),
                        value: value.to_owned(),
                    }],
                }],
                "input = {input:?}"
            );
        }
        assert_eq!(
            parse("&bogus;").children,
            vec![Block::Paragraph {
                span: span(0, 7),
                content: vec![Inline::Text { span: span(0, 7) }],
            }]
        );
    }

    #[test]
    fn code_spans_support_exact_runs_embedded_backticks_and_unmatched_openers() {
        assert_eq!(
            parse("`code`").children,
            vec![Block::Paragraph {
                span: span(0, 6),
                content: vec![Inline::CodeSpan {
                    span: span(0, 6),
                    literal: vec![span(1, 5)],
                }],
            }]
        );
        assert_eq!(
            parse("``a`b``").children,
            vec![Block::Paragraph {
                span: span(0, 7),
                content: vec![Inline::CodeSpan {
                    span: span(0, 7),
                    literal: vec![span(2, 5)],
                }],
            }]
        );
        assert_eq!(
            parse("`open").children,
            vec![Block::Paragraph {
                span: span(0, 5),
                content: vec![Inline::Text { span: span(0, 5) }],
            }]
        );
    }

    #[test]
    fn code_span_can_cross_fragments_and_trims_balanced_spaces() {
        assert_eq!(
            parse("`a\nb`").children,
            vec![Block::Paragraph {
                span: span(0, 5),
                content: vec![Inline::CodeSpan {
                    span: span(0, 5),
                    literal: vec![span(1, 2), span(3, 4)],
                }],
            }]
        );
        assert_eq!(
            parse("` x `").children,
            vec![Block::Paragraph {
                span: span(0, 5),
                content: vec![Inline::CodeSpan {
                    span: span(0, 5),
                    literal: vec![span(2, 3)],
                }],
            }]
        );
        assert_eq!(
            parse("`*a*`").children,
            vec![Block::Paragraph {
                span: span(0, 5),
                content: vec![Inline::CodeSpan {
                    span: span(0, 5),
                    literal: vec![span(1, 4)],
                }],
            }]
        );
    }

    #[test]
    fn emphasis_and_strong_follow_flanking_rules() {
        let cases = [
            (
                "*foo*",
                Inline::Emphasis {
                    span: span(0, 5),
                    children: vec![Inline::Text { span: span(1, 4) }],
                },
            ),
            (
                "**bold**",
                Inline::Strong {
                    span: span(0, 8),
                    children: vec![Inline::Text { span: span(2, 6) }],
                },
            ),
            (
                "_under_",
                Inline::Emphasis {
                    span: span(0, 7),
                    children: vec![Inline::Text { span: span(1, 6) }],
                },
            ),
        ];
        for (input, inline) in cases {
            assert_eq!(
                parse(input).children,
                vec![Block::Paragraph {
                    span: span(0, input.len() as u32),
                    content: vec![inline],
                }],
                "input = {input:?}"
            );
        }

        assert_eq!(
            parse("foo*bar*baz").children,
            vec![Block::Paragraph {
                span: span(0, 11),
                content: vec![
                    Inline::Text { span: span(0, 3) },
                    Inline::Emphasis {
                        span: span(3, 8),
                        children: vec![Inline::Text { span: span(4, 7) }],
                    },
                    Inline::Text { span: span(8, 11) },
                ],
            }]
        );
        for input in ["foo_bar_baz", "foo * bar", "*foo"] {
            assert_eq!(
                parse(input).children,
                vec![Block::Paragraph {
                    span: span(0, input.len() as u32),
                    content: vec![Inline::Text {
                        span: span(0, input.len() as u32),
                    }],
                }],
                "input = {input:?}"
            );
        }
        assert_eq!(
            parse("«_x_»").children,
            vec![Block::Paragraph {
                span: span(0, 7),
                content: vec![
                    Inline::Text { span: span(0, 2) },
                    Inline::Emphasis {
                        span: span(2, 5),
                        children: vec![Inline::Text { span: span(3, 4) }],
                    },
                    Inline::Text { span: span(5, 7) },
                ],
            }]
        );
    }

    #[test]
    fn emphasis_supports_triple_runs_leftovers_and_rule_of_three() {
        assert_eq!(
            parse("***both***").children,
            vec![Block::Paragraph {
                span: span(0, 10),
                content: vec![Inline::Emphasis {
                    span: span(0, 10),
                    children: vec![Inline::Strong {
                        span: span(1, 9),
                        children: vec![Inline::Text { span: span(3, 7) }],
                    }],
                }],
            }]
        );
        assert_eq!(
            parse("**foo*").children,
            vec![Block::Paragraph {
                span: span(0, 6),
                content: vec![
                    Inline::Text { span: span(0, 1) },
                    Inline::Emphasis {
                        span: span(1, 6),
                        children: vec![Inline::Text { span: span(2, 5) }],
                    },
                ],
            }]
        );
        assert_eq!(
            parse("*foo**bar*").children,
            vec![Block::Paragraph {
                span: span(0, 10),
                content: vec![Inline::Emphasis {
                    span: span(0, 10),
                    children: vec![Inline::Text { span: span(1, 9) }],
                }],
            }]
        );
    }

    #[test]
    fn unified_stack_preserves_emphasis_regression_sentinels() {
        assert_eq!(
            parse("*foo**").children,
            vec![Block::Paragraph {
                span: span(0, 6),
                content: vec![
                    Inline::Emphasis {
                        span: span(0, 5),
                        children: vec![Inline::Text { span: span(1, 4) }],
                    },
                    Inline::Text { span: span(5, 6) },
                ],
            }]
        );
        assert_eq!(
            parse("_foo_bar_baz_").children,
            vec![Block::Paragraph {
                span: span(0, 13),
                content: vec![Inline::Emphasis {
                    span: span(0, 13),
                    children: vec![Inline::Text { span: span(1, 12) }],
                }],
            }]
        );
        assert_eq!(
            parse("*(*foo*)*").children,
            vec![Block::Paragraph {
                span: span(0, 9),
                content: vec![Inline::Emphasis {
                    span: span(0, 9),
                    children: vec![
                        Inline::Text { span: span(1, 2) },
                        Inline::Emphasis {
                            span: span(2, 7),
                            children: vec![Inline::Text { span: span(3, 6) }],
                        },
                        Inline::Text { span: span(7, 8) },
                    ],
                }],
            }]
        );
        assert_eq!(
            parse("5*6*78").children,
            vec![Block::Paragraph {
                span: span(0, 6),
                content: vec![
                    Inline::Text { span: span(0, 1) },
                    Inline::Emphasis {
                        span: span(1, 4),
                        children: vec![Inline::Text { span: span(2, 3) }],
                    },
                    Inline::Text { span: span(4, 6) },
                ],
            }]
        );
    }

    #[test]
    fn inline_links_images_and_titles_have_exact_source_spans() {
        let cases = [
            ("[a](b)", span(4, 5), None),
            ("[a](b \"title\")", span(4, 5), Some(span(7, 12))),
            ("[a](b 'title')", span(4, 5), Some(span(7, 12))),
            ("[a](b (title))", span(4, 5), Some(span(7, 12))),
            ("[a](<dest with space>)", span(5, 20), None),
            ("[a](b(c)d)", span(4, 9), None),
        ];
        for (input, dest, title) in cases {
            assert_eq!(
                parse(input).children,
                vec![Block::Paragraph {
                    span: span(0, input.len() as u32),
                    content: vec![Inline::Link {
                        span: span(0, input.len() as u32),
                        target: LinkTarget {
                            dest: Some(dest),
                            title,
                            label: None,
                        },
                        children: vec![Inline::Text { span: span(1, 2) }],
                    }],
                }],
                "input = {input:?}"
            );
        }

        assert_eq!(
            parse("[a]()").children,
            vec![Block::Paragraph {
                span: span(0, 5),
                content: vec![Inline::Link {
                    span: span(0, 5),
                    target: LinkTarget {
                        dest: None,
                        title: None,
                        label: None,
                    },
                    children: vec![Inline::Text { span: span(1, 2) }],
                }],
            }]
        );
        assert_eq!(
            parse("![alt](img)").children,
            vec![Block::Paragraph {
                span: span(0, 11),
                content: vec![Inline::Image {
                    span: span(0, 11),
                    target: LinkTarget {
                        dest: Some(span(7, 10)),
                        title: None,
                        label: None,
                    },
                    children: vec![Inline::Text { span: span(2, 5) }],
                }],
            }]
        );
        assert_eq!(
            parse("[a](b \"ti\ntle\")").children,
            vec![Block::Paragraph {
                span: span(0, 15),
                content: vec![Inline::Link {
                    span: span(0, 15),
                    target: LinkTarget {
                        dest: Some(span(4, 5)),
                        title: Some(span(7, 13)),
                        label: None,
                    },
                    children: vec![Inline::Text { span: span(1, 2) }],
                }],
            }]
        );
    }

    #[test]
    fn inline_links_interact_with_emphasis_and_bracket_deactivation() {
        assert_eq!(
            parse("[*a*](b)").children,
            vec![Block::Paragraph {
                span: span(0, 8),
                content: vec![Inline::Link {
                    span: span(0, 8),
                    target: LinkTarget {
                        dest: Some(span(6, 7)),
                        title: None,
                        label: None,
                    },
                    children: vec![Inline::Emphasis {
                        span: span(1, 4),
                        children: vec![Inline::Text { span: span(2, 3) }],
                    }],
                }],
            }]
        );
        assert!(matches!(
            &parse("*[a](b)*").children[0],
            Block::Paragraph { content, .. }
                if matches!(&content[..], [Inline::Emphasis { children, .. }]
                    if matches!(&children[..], [Inline::Link { .. }]))
        ));
        assert!(matches!(
            &parse("[a [b](c) d](e)").children[0],
            Block::Paragraph { content, .. }
                if matches!(&content[..], [Inline::Text { .. }, Inline::Link { .. }, Inline::Text { .. }])
        ));
        assert!(matches!(
            &parse("[a ![b](c)](d)").children[0],
            Block::Paragraph { content, .. }
                if matches!(&content[..], [Inline::Link { children, .. }]
                    if matches!(&children[..], [Inline::Text { .. }, Inline::Image { .. }]))
        ));
    }

    #[test]
    fn reference_links_support_full_collapsed_shortcut_and_normalization() {
        let input = "[a][ref] [ref][] [ref]\n\n[ ReF ]: /dest \"title\"";
        let document = parse(input);
        assert_eq!(
            document.definitions,
            vec![LinkDefinition {
                span: span(24, 46),
                label: span(25, 30),
                dest: span(33, 38),
                title: Some(span(40, 45)),
            }]
        );
        let Block::Paragraph { content, .. } = &document.children[0] else {
            panic!("expected paragraph");
        };
        let links = content
            .iter()
            .filter_map(|inline| match inline {
                Inline::Link { target, .. } => Some(*target),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            links,
            vec![
                LinkTarget {
                    dest: Some(span(33, 38)),
                    title: Some(span(40, 45)),
                    label: Some(span(4, 7)),
                },
                LinkTarget {
                    dest: Some(span(33, 38)),
                    title: Some(span(40, 45)),
                    label: Some(span(10, 13)),
                },
                LinkTarget {
                    dest: Some(span(33, 38)),
                    title: Some(span(40, 45)),
                    label: Some(span(18, 21)),
                },
            ]
        );

        let normalized = parse("[shown][ foo\t BAR ]\n\n[FOO  bar]: /x");
        assert!(matches!(
            &normalized.children[0],
            Block::Paragraph { content, .. }
                if matches!(&content[..], [Inline::Link { target: LinkTarget { dest: Some(Span { start: 33, end: 35 }), .. }, .. }])
        ));
    }

    #[test]
    fn definitions_are_global_first_wins_and_definition_only_blocks_disappear() {
        let input = "[use]\n\n[use]: first\n\n[USE]: second";
        let document = parse(input);
        assert_eq!(document.definitions.len(), 2);
        assert_eq!(document.definitions[0].dest, span(14, 19));
        assert_eq!(document.definitions[1].dest, span(28, 34));
        assert_eq!(document.children.len(), 1);
        assert!(matches!(
            &document.children[0],
            Block::Paragraph { content, .. }
                if matches!(&content[..], [Inline::Link { target: LinkTarget { dest: Some(Span { start: 14, end: 19 }), .. }, .. }])
        ));

        let unresolved = parse("[missing]");
        assert_eq!(
            unresolved.children,
            vec![Block::Paragraph {
                span: span(0, 9),
                content: vec![Inline::Text { span: span(0, 9) }],
            }]
        );
    }

    #[test]
    fn link_definition_labels_can_span_multiple_lines() {
        let input = "[\nfoo\n]: /url\nbar";
        let document = parse(input);

        assert_eq!(document.definitions.len(), 1);
        let definition = document.definitions[0];
        assert_eq!(definition.span.slice(input), "[\nfoo\n]: /url");
        assert_eq!(definition.label.slice(input), "\nfoo\n");
        assert_eq!(definition.dest.slice(input), "/url");
        assert_eq!(definition.title, None);
        assert_eq!(
            document.children,
            vec![Block::Paragraph {
                span: span(14, 17),
                content: vec![Inline::Text { span: span(14, 17) }],
            }]
        );
    }

    #[test]
    fn link_definition_destination_can_start_on_the_next_line() {
        let input = "[foo]:\n/url\nrest";
        let document = parse(input);

        assert_eq!(document.definitions.len(), 1);
        let definition = document.definitions[0];
        assert_eq!(definition.span.slice(input), "[foo]:\n/url");
        assert_eq!(definition.label.slice(input), "foo");
        assert_eq!(definition.dest.slice(input), "/url");
        assert_eq!(definition.title, None);
        assert!(matches!(
            &document.children[..],
            [Block::Paragraph {
                span: Span { start: 12, end: 16 },
                ..
            }]
        ));
    }

    #[test]
    fn link_definition_title_can_start_on_the_next_line() {
        let input = "[foo]: /url\n  \"title\"\nrest";
        let document = parse(input);

        assert_eq!(document.definitions.len(), 1);
        let definition = document.definitions[0];
        assert_eq!(definition.span.slice(input), "[foo]: /url\n  \"title\"");
        assert_eq!(definition.dest.slice(input), "/url");
        assert_eq!(
            definition.title.map(|title| title.slice(input)),
            Some("title")
        );
        assert!(matches!(
            &document.children[..],
            [Block::Paragraph {
                span: Span { start: 22, end: 26 },
                ..
            }]
        ));
    }

    #[test]
    fn link_definition_titles_can_span_multiple_lines() {
        let input = "[foo]: /url '\ntitle\nline1\nline2\n'\nrest";
        let document = parse(input);

        assert_eq!(document.definitions.len(), 1);
        let definition = document.definitions[0];
        assert_eq!(
            definition.title.map(|title| title.slice(input)),
            Some("\ntitle\nline1\nline2\n")
        );
        assert_eq!(
            definition.span.slice(input),
            "[foo]: /url '\ntitle\nline1\nline2\n'"
        );
        assert!(matches!(
            &document.children[..],
            [Block::Paragraph { content, .. }]
                if matches!(&content[..], [Inline::Text { span }]
                    if span.slice(input) == "rest")
        ));
    }

    #[test]
    fn link_definition_continuation_lines_allow_leading_indentation() {
        let input = "   [foo]:\n   /url\n   'title'\nrest";
        let document = parse(input);

        assert_eq!(document.definitions.len(), 1);
        let definition = document.definitions[0];
        assert_eq!(definition.span.start, 3);
        assert_eq!(definition.dest.slice(input), "/url");
        assert_eq!(
            definition.title.map(|title| title.slice(input)),
            Some("title")
        );
        assert!(matches!(
            &document.children[..],
            [Block::Paragraph { content, .. }]
                if matches!(&content[..], [Inline::Text { span }]
                    if span.slice(input) == "rest")
        ));
    }

    #[test]
    fn malformed_multiline_link_definition_falls_back_atomically() {
        let input = "[foo]:\n/url \"unterminated\nrest";
        let document = parse(input);

        assert!(document.definitions.is_empty());
        assert!(matches!(
            &document.children[..],
            [Block::Paragraph {
                span: Span { start: 0, end: 30 },
                ..
            }]
        ));

        let whitespace_label = parse("[\n ]: /url");
        assert!(whitespace_label.definitions.is_empty());
        assert!(matches!(
            &whitespace_label.children[..],
            [Block::Paragraph { .. }]
        ));

        let nested_parenthesis = parse("[foo]: /url (a (b)\n\n[foo]");
        assert!(nested_parenthesis.definitions.is_empty());
    }

    #[test]
    fn link_definition_leaves_invalid_next_line_title_in_the_paragraph() {
        let input = "[foo]: /url\n\"title\" ok\n[foo]";
        let document = parse(input);

        assert_eq!(document.definitions.len(), 1);
        assert_eq!(document.definitions[0].span.slice(input), "[foo]: /url");
        assert_eq!(document.definitions[0].dest.slice(input), "/url");
        assert!(matches!(
            &document.children[..],
            [Block::Paragraph { span: Span { start: 12, end: 28 }, content }]
                if matches!(&content[..],
                    [Inline::Text { span: first }, Inline::SoftBreak { .. }, Inline::Link { target, .. }]
                    if first.slice(input) == "\"title\" ok"
                        && target.dest.is_some_and(|dest| dest.slice(input) == "/url"))
        ));
    }

    #[test]
    fn duplicate_multiline_link_definitions_keep_the_first_target() {
        let input = "[dup]:\n/first\n[dup]: /second\n\n[dup]";
        let document = parse(input);

        assert_eq!(document.definitions.len(), 2);
        assert_eq!(document.definitions[0].dest.slice(input), "/first");
        assert_eq!(document.definitions[1].dest.slice(input), "/second");
        assert!(matches!(
            &document.children[..],
            [Block::Paragraph { content, .. }]
                if matches!(&content[..], [Inline::Link { target, .. }]
                    if target.dest.is_some_and(|dest| dest.slice(input) == "/first"))
        ));
    }

    #[test]
    fn setext_headings_extract_leading_multiline_link_definitions() {
        let input = "[\nfoo\n]: /url\nbar\n===\n\n[foo]";
        let document = parse(input);

        assert_eq!(document.definitions.len(), 1);
        assert_eq!(document.definitions[0].label.slice(input), "\nfoo\n");
        assert_eq!(document.definitions[0].dest.slice(input), "/url");
        assert!(matches!(
            &document.children[..],
            [Block::Heading { kind: HeadingKind::Setext, content, .. },
             Block::Paragraph { content: reference, .. }]
                if matches!(&content[..], [Inline::Text { span }] if span.slice(input) == "bar")
                    && matches!(&reference[..], [Inline::Link { target, .. }]
                        if target.dest.is_some_and(|dest| dest.slice(input) == "/url"))
        ));
    }

    #[test]
    fn malformed_full_reference_falls_back_only_to_a_defined_shortcut() {
        let malformed = parse("[foo][bar\n\n[foo]: /url");
        assert_eq!(
            malformed.children,
            vec![Block::Paragraph {
                span: span(0, 9),
                content: vec![
                    Inline::Link {
                        span: span(0, 5),
                        target: LinkTarget {
                            dest: Some(span(18, 22)),
                            title: None,
                            label: Some(span(1, 4)),
                        },
                        children: vec![Inline::Text { span: span(1, 4) }],
                    },
                    Inline::Text { span: span(5, 9) },
                ],
            }]
        );

        let unresolved = parse("[foo][bar]\n\n[foo]: /url");
        assert_eq!(
            unresolved.children,
            vec![Block::Paragraph {
                span: span(0, 10),
                content: vec![Inline::Text { span: span(0, 10) }],
            }]
        );
    }

    #[test]
    fn footnote_definition_stays_in_place_and_resolves_case_insensitively() {
        assert_eq!(
            parse("ref[^A]\n\n[^a]: note").children,
            vec![
                Block::Paragraph {
                    span: span(0, 7),
                    content: vec![
                        Inline::Text { span: span(0, 3) },
                        Inline::FootnoteReference {
                            span: span(3, 7),
                            label: span(5, 6),
                        },
                    ],
                },
                Block::FootnoteDefinition {
                    span: span(9, 19),
                    label: span(11, 12),
                    children: vec![Block::Paragraph {
                        span: span(15, 19),
                        content: vec![Inline::Text { span: span(15, 19) }],
                    }],
                },
            ]
        );
    }

    #[test]
    fn footnotes_support_lazy_paragraphs_duplicates_and_literal_fallback() {
        assert_eq!(
            parse("[^n]: first\nsecond").children,
            vec![Block::FootnoteDefinition {
                span: span(0, 18),
                label: span(2, 3),
                children: vec![Block::Paragraph {
                    span: span(6, 18),
                    content: vec![
                        Inline::Text { span: span(6, 11) },
                        Inline::SoftBreak { span: span(11, 11) },
                        Inline::Text { span: span(12, 18) },
                    ],
                }],
            }]
        );

        let duplicate = parse("[^a]: first\n\n[^A]: second\n\n[^a]");
        assert!(matches!(
            &duplicate.children[..],
            [Block::FootnoteDefinition { label: Span { start: 2, end: 3 }, .. },
             Block::FootnoteDefinition { label: Span { start: 15, end: 16 }, .. },
             Block::Paragraph { content, .. }]
                if matches!(&content[..], [Inline::FootnoteReference { .. }])
        ));

        assert_eq!(
            parse("[^missing]").children,
            vec![Block::Paragraph {
                span: span(0, 10),
                content: vec![Inline::Text { span: span(0, 10) }],
            }]
        );
    }

    #[test]
    fn autolinks_recognize_uris_and_email_addresses() {
        assert_eq!(
            parse("<https://x.y> <a@b.c> <foo>").children,
            vec![Block::Paragraph {
                span: span(0, 27),
                content: vec![
                    Inline::Autolink {
                        span: span(0, 13),
                        uri: span(1, 12),
                        email: false,
                    },
                    Inline::Text { span: span(13, 14) },
                    Inline::Autolink {
                        span: span(14, 21),
                        uri: span(15, 20),
                        email: true,
                    },
                    Inline::Text { span: span(21, 27) },
                ],
            }]
        );
    }

    #[test]
    fn literal_autolinks_recognize_urls_email_and_boundaries() {
        let cases = [
            ("www.a.com", false),
            ("https://a.com/path", false),
            ("a@b.com", true),
        ];
        for (input, email) in cases {
            assert_eq!(
                parse(input).children,
                vec![Block::Paragraph {
                    span: span(0, input.len() as u32),
                    content: vec![Inline::Autolink {
                        span: span(0, input.len() as u32),
                        uri: span(0, input.len() as u32),
                        email,
                    }],
                }],
                "input = {input:?}"
            );
        }

        assert_eq!(
            parse("(see www.a.com/x)").children,
            vec![Block::Paragraph {
                span: span(0, 17),
                content: vec![
                    Inline::Text { span: span(0, 5) },
                    Inline::Autolink {
                        span: span(5, 16),
                        uri: span(5, 16),
                        email: false,
                    },
                    Inline::Text { span: span(16, 17) },
                ],
            }]
        );
        assert_eq!(
            parse("(www.a.com)").children,
            vec![Block::Paragraph {
                span: span(0, 11),
                content: vec![
                    Inline::Text { span: span(0, 1) },
                    Inline::Autolink {
                        span: span(1, 10),
                        uri: span(1, 10),
                        email: false,
                    },
                    Inline::Text { span: span(10, 11) },
                ],
            }]
        );
        assert!(matches!(
            &parse("*www.a.com*").children[0],
            Block::Paragraph { content, .. }
                if matches!(&content[..], [Inline::Emphasis { children, .. }]
                    if matches!(&children[..], [Inline::Autolink { .. }]))
        ));
    }

    #[test]
    fn literal_autolinks_trim_punctuation_and_reject_midword_matches() {
        assert_eq!(
            parse("www.a.com. www.b.com,").children,
            vec![Block::Paragraph {
                span: span(0, 21),
                content: vec![
                    Inline::Autolink {
                        span: span(0, 9),
                        uri: span(0, 9),
                        email: false,
                    },
                    Inline::Text { span: span(9, 11) },
                    Inline::Autolink {
                        span: span(11, 20),
                        uri: span(11, 20),
                        email: false,
                    },
                    Inline::Text { span: span(20, 21) },
                ],
            }]
        );
        for input in ["xwww.a.com", "w.a.com"] {
            assert_eq!(
                parse(input).children,
                vec![Block::Paragraph {
                    span: span(0, input.len() as u32),
                    content: vec![Inline::Text {
                        span: span(0, input.len() as u32),
                    }],
                }]
            );
        }
    }

    #[test]
    fn wikilinks_precede_brackets_and_do_not_cross_fragments() {
        let cases = [
            (
                "[[Note]]",
                vec![Inline::WikiLink {
                    span: span(0, 8),
                    target: span(2, 6),
                    label: None,
                }],
            ),
            (
                "[[Note|shown]]",
                vec![Inline::WikiLink {
                    span: span(0, 14),
                    target: span(2, 6),
                    label: Some(span(7, 12)),
                }],
            ),
            (
                "[[x]](y)",
                vec![
                    Inline::WikiLink {
                        span: span(0, 5),
                        target: span(2, 3),
                        label: None,
                    },
                    Inline::Text { span: span(5, 8) },
                ],
            ),
        ];
        for (input, content) in cases {
            assert_eq!(
                parse(input).children,
                vec![Block::Paragraph {
                    span: span(0, input.len() as u32),
                    content,
                }]
            );
        }
        for input in ["[[unclosed", "[[]]", "[[a\nb]]"] {
            assert!(matches!(
                &parse(input).children[0],
                Block::Paragraph { content, .. }
                    if !content.iter().any(|inline| matches!(inline, Inline::WikiLink { .. }))
            ));
        }
    }

    #[test]
    fn strikethrough_and_highlight_follow_flanking_and_nest_with_emphasis() {
        assert_eq!(
            parse("~~x~~").children,
            vec![Block::Paragraph {
                span: span(0, 5),
                content: vec![Inline::Strikethrough {
                    span: span(0, 5),
                    children: vec![Inline::Text { span: span(2, 3) }],
                }],
            }]
        );
        assert_eq!(
            parse("~x~").children,
            vec![Block::Paragraph {
                span: span(0, 3),
                content: vec![Inline::Strikethrough {
                    span: span(0, 3),
                    children: vec![Inline::Text { span: span(1, 2) }],
                }],
            }]
        );
        assert_eq!(
            parse("==mark==").children,
            vec![Block::Paragraph {
                span: span(0, 8),
                content: vec![Inline::Highlight {
                    span: span(0, 8),
                    children: vec![Inline::Text { span: span(2, 6) }],
                }],
            }]
        );
        let cases = [
            ("~~x~~", true),
            ("~x~", true),
            ("==mark==", true),
            ("=x=", false),
            ("a == b", false),
            ("a ~~~x~~~", false),
        ];
        for (input, participates) in cases {
            let Block::Paragraph { content, .. } = &parse(input).children[0] else {
                panic!("expected paragraph for {input:?}");
            };
            let found = content.iter().any(|inline| {
                matches!(
                    inline,
                    Inline::Strikethrough { .. } | Inline::Highlight { .. }
                )
            });
            assert_eq!(found, participates, "input = {input:?}");
        }
        assert!(matches!(
            &parse("*~~x~~*").children[0],
            Block::Paragraph { content, .. }
                if matches!(&content[..], [Inline::Emphasis { children, .. }]
                    if matches!(&children[..], [Inline::Strikethrough { .. }]))
        ));
        assert!(matches!(
            &parse("~~*x*~~").children[0],
            Block::Paragraph { content, .. }
                if matches!(&content[..], [Inline::Strikethrough { children, .. }]
                    if matches!(&children[..], [Inline::Emphasis { .. }]))
        ));
    }

    #[test]
    fn mixed_length_tilde_runs_pair_across_a_failed_shorter_closer() {
        // `~~a~b~~`: the interior length-1 tilde fails to pair, but its
        // failure must not advance the bottom past the length-2 opener —
        // cmark-gfm pairs the outer runs: <del>a~b</del>.
        assert_eq!(
            parse("~~a~b~~").children,
            vec![Block::Paragraph {
                span: span(0, 7),
                content: vec![Inline::Strikethrough {
                    span: span(0, 7),
                    children: vec![Inline::Text { span: span(2, 5) }],
                }],
            }]
        );
    }

    #[test]
    fn crossing_delimiters_follow_single_stack_closer_order() {
        assert_eq!(
            parse("*a ~~b* c~~").children,
            vec![Block::Paragraph {
                span: span(0, 11),
                content: vec![
                    Inline::Emphasis {
                        span: span(0, 7),
                        children: vec![Inline::Text { span: span(1, 6) }],
                    },
                    Inline::Text { span: span(7, 11) },
                ],
            }]
        );
        assert_eq!(
            parse("~~a *b~~ c*").children,
            vec![Block::Paragraph {
                span: span(0, 11),
                content: vec![
                    Inline::Strikethrough {
                        span: span(0, 8),
                        children: vec![Inline::Text { span: span(2, 6) }],
                    },
                    Inline::Text { span: span(8, 11) },
                ],
            }]
        );
        for input in ["~~*em*~~", "*~~x~~*", "==*a*=="] {
            let Block::Paragraph { content, .. } = &parse(input).children[0] else {
                panic!("expected paragraph");
            };
            assert_eq!(content.len(), 1, "input = {input:?}");
            assert!(matches!(
                &content[0],
                Inline::Strikethrough { children, .. }
                    | Inline::Emphasis { children, .. }
                    | Inline::Highlight { children, .. }
                    if matches!(&children[..], [Inline::Emphasis { .. } | Inline::Strikethrough { .. }])
            ));
        }
    }

    #[test]
    fn delimiter_processing_handles_pathological_alternating_input_quickly() {
        let input = "*a".repeat(2_000);
        let started = std::time::Instant::now();
        let document = parse(&input);
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        assert!(matches!(&document.children[..], [Block::Paragraph { .. }]));
    }

    #[test]
    fn math_obeys_adjacency_and_preserves_literal_fragment_spans() {
        assert_eq!(
            parse("$x$ $$y$$").children,
            vec![Block::Paragraph {
                span: span(0, 9),
                content: vec![
                    Inline::Math {
                        span: span(0, 3),
                        display: false,
                        literal: vec![span(1, 2)],
                    },
                    Inline::Text { span: span(3, 4) },
                    Inline::Math {
                        span: span(4, 9),
                        display: true,
                        literal: vec![span(6, 7)],
                    },
                ],
            }]
        );
        for input in ["$ x$", "$x $", "$x$5", "$5 and $6"] {
            assert_eq!(
                parse(input).children,
                vec![Block::Paragraph {
                    span: span(0, input.len() as u32),
                    content: vec![Inline::Text {
                        span: span(0, input.len() as u32),
                    }],
                }],
                "input = {input:?}"
            );
        }
        assert_eq!(
            parse("$$\nx\n$$").children,
            vec![Block::Paragraph {
                span: span(0, 7),
                content: vec![Inline::Math {
                    span: span(0, 7),
                    display: true,
                    literal: vec![span(2, 2), span(3, 4), span(5, 5)],
                }],
            }]
        );
    }

    #[test]
    fn fragment_boundaries_produce_soft_and_hard_breaks() {
        let cases = [
            (
                "a\nb",
                vec![
                    Inline::Text { span: span(0, 1) },
                    Inline::SoftBreak { span: span(1, 1) },
                    Inline::Text { span: span(2, 3) },
                ],
            ),
            (
                "a  \nb",
                vec![
                    Inline::Text { span: span(0, 1) },
                    Inline::HardBreak { span: span(1, 3) },
                    Inline::Text { span: span(4, 5) },
                ],
            ),
            (
                "a\\\nb",
                vec![
                    Inline::Text { span: span(0, 1) },
                    Inline::HardBreak { span: span(1, 2) },
                    Inline::Text { span: span(3, 4) },
                ],
            ),
            (
                "a \nb",
                vec![
                    Inline::Text { span: span(0, 1) },
                    Inline::SoftBreak { span: span(1, 1) },
                    Inline::Text { span: span(3, 4) },
                ],
            ),
        ];
        for (input, content) in cases {
            assert_eq!(
                parse(input).children,
                vec![Block::Paragraph {
                    span: span(0, input.len() as u32),
                    content,
                }],
                "input = {input:?}"
            );
        }
        assert_eq!(
            parse("a  ").children,
            vec![Block::Paragraph {
                span: span(0, 3),
                content: vec![Inline::Text { span: span(0, 1) }],
            }]
        );
    }

    #[test]
    fn headings_and_container_leaf_blocks_receive_inline_content() {
        assert_eq!(
            parse("# *hi*").children,
            vec![Block::Heading {
                span: span(0, 6),
                level: 1,
                kind: HeadingKind::Atx,
                content: vec![Inline::Emphasis {
                    span: span(2, 6),
                    children: vec![Inline::Text { span: span(3, 5) }],
                }],
            }]
        );
        assert_eq!(
            parse("a\n*b*\n---").children,
            vec![Block::Heading {
                span: span(0, 9),
                level: 2,
                kind: HeadingKind::Setext,
                content: vec![
                    Inline::Text { span: span(0, 1) },
                    Inline::SoftBreak { span: span(1, 1) },
                    Inline::Emphasis {
                        span: span(2, 5),
                        children: vec![Inline::Text { span: span(3, 4) }],
                    },
                ],
            }]
        );
        assert_eq!(
            parse("> *a*").children,
            vec![block_quote(
                span(0, 5),
                vec![Block::Paragraph {
                    span: span(2, 5),
                    content: vec![Inline::Emphasis {
                        span: span(2, 5),
                        children: vec![Inline::Text { span: span(3, 4) }],
                    }],
                }],
            )]
        );
        assert_eq!(
            parse("- **b**").children,
            vec![list(
                span(0, 7),
                ListKind::Bullet { marker: b'-' },
                true,
                vec![item(
                    span(0, 7),
                    vec![Block::Paragraph {
                        span: span(2, 7),
                        content: vec![Inline::Strong {
                            span: span(2, 7),
                            children: vec![Inline::Text { span: span(4, 5) }],
                        }],
                    }],
                )],
            )]
        );
    }

    #[test]
    fn basic_table_has_exact_rows_cells_and_alignments() {
        assert_eq!(
            parse("| a | b |\n| :- | -: |\n| x | y |").children,
            vec![Block::Table {
                span: span(0, 31),
                alignments: vec![TableAlignment::Left, TableAlignment::Right],
                head: TableRow {
                    span: span(0, 9),
                    cells: vec![
                        TableCell {
                            span: span(2, 3),
                            content: vec![Inline::Text { span: span(2, 3) }],
                        },
                        TableCell {
                            span: span(6, 7),
                            content: vec![Inline::Text { span: span(6, 7) }],
                        },
                    ],
                },
                rows: vec![TableRow {
                    span: span(22, 31),
                    cells: vec![
                        TableCell {
                            span: span(24, 25),
                            content: vec![Inline::Text { span: span(24, 25) }],
                        },
                        TableCell {
                            span: span(28, 29),
                            content: vec![Inline::Text { span: span(28, 29) }],
                        },
                    ],
                }],
            }]
        );
    }

    #[test]
    fn tables_support_all_alignments_and_normalize_row_width() {
        let document = parse("a|b|c|d\n---|:---|:---:|---:\none\nx|y|z|w|dropped");
        let Block::Table {
            alignments, rows, ..
        } = &document.children[0]
        else {
            panic!("expected table");
        };
        assert_eq!(
            alignments,
            &[
                TableAlignment::None,
                TableAlignment::Left,
                TableAlignment::Center,
                TableAlignment::Right,
            ]
        );
        assert_eq!(rows[0].cells.len(), 4);
        assert_eq!(
            rows[0].cells[0].content,
            vec![Inline::Text { span: span(28, 31) }]
        );
        assert!(
            rows[0].cells[1..]
                .iter()
                .all(|cell| cell.span == span(31, 31) && cell.content.is_empty())
        );
        assert_eq!(rows[1].cells.len(), 4);
        assert_eq!(
            rows[1].cells[3].content,
            vec![Inline::Text { span: span(38, 39) }]
        );
    }

    #[test]
    fn table_validation_handles_mismatch_termination_and_containers() {
        let mismatch = parse("a | b\n--- | --- | ---");
        assert!(matches!(&mismatch.children[..], [Block::Paragraph { .. }]));

        let terminated = parse("a|b\n-|-\nx|y\n\n# end");
        assert!(matches!(
            &terminated.children[..],
            [Block::Table { rows, .. }, Block::Heading { .. }] if rows.len() == 1
        ));

        let quote = parse("> | a | b |\n> | - | - |\n> | x | y |");
        assert!(matches!(
            &quote.children[..],
            [Block::BlockQuote { children, .. }]
                if matches!(&children[..], [Block::Table { .. }])
        ));
        let list_table = parse("- a|b\n  -|-\n  x|y");
        assert!(matches!(
            &list_table.children[..],
            [Block::List { items, .. }]
                if matches!(&items[0].children[..], [Block::Table { .. }])
        ));
    }

    #[test]
    fn tables_split_escaped_pipes_and_parse_inline_cell_content() {
        let document = parse("intro\n*a*|`b`|[[C]]\n---|---|---\nx \\| y|z|w");
        assert!(matches!(&document.children[0], Block::Paragraph { .. }));
        let Block::Table { head, rows, .. } = &document.children[1] else {
            panic!("expected table after paragraph prefix");
        };
        assert!(matches!(
            &head.cells[0].content[..],
            [Inline::Emphasis { .. }]
        ));
        assert!(matches!(
            &head.cells[1].content[..],
            [Inline::CodeSpan { .. }]
        ));
        assert!(matches!(
            &head.cells[2].content[..],
            [Inline::WikiLink { .. }]
        ));
        assert_eq!(rows[0].cells.len(), 3);
        assert!(matches!(
            &rows[0].cells[0].content[..],
            [
                Inline::Text { .. },
                Inline::Escaped { .. },
                Inline::Text { .. }
            ]
        ));
    }

    #[test]
    fn terminated_frontmatter_is_stored_and_body_is_parsed() {
        let input = "---\ntitle: Notes\n---\nbody";
        let body_start = "---\ntitle: Notes\n---\n".len() as u32;
        assert_eq!(
            parse(input),
            Document {
                frontmatter: Some(FrontmatterBlock {
                    span: Span {
                        start: 0,
                        end: body_start,
                    },
                    terminated: true,
                }),
                children: vec![paragraph(
                    Span {
                        start: body_start,
                        end: input.len() as u32,
                    },
                    vec![Span {
                        start: body_start,
                        end: input.len() as u32,
                    }],
                )],
                definitions: Vec::new(),
                diagnostics: Vec::new(),
            }
        );
    }

    #[test]
    fn unterminated_frontmatter_emits_a_diagnostic_and_has_no_children() {
        let input = "---\ntitle: Notes\n";
        let span = Span {
            start: 0,
            end: input.len() as u32,
        };
        assert_eq!(
            parse(input),
            Document {
                frontmatter: Some(FrontmatterBlock {
                    span,
                    terminated: false,
                }),
                children: Vec::new(),
                definitions: Vec::new(),
                diagnostics: vec![Diagnostic {
                    span,
                    kind: DiagnosticKind::UnterminatedFrontmatter,
                }],
            }
        );
    }

    #[test]
    fn all_parser_spans_are_ordered_non_overlapping_and_on_char_boundaries() {
        let inputs = [
            "",
            "plain text",
            "  indented  \ncontinuation",
            "one\n\ntwo",
            "one\r\n\r\ntwo",
            "café ☕\n🦀",
            "# heading",
            "title\n---",
            "\n***",
            "``` rust\n# literal\n```",
            "~~~\nbody",
            "    code\n\n    more",
            "> a\r\n> b",
            "- a\n  - b\n- c",
            "---\ntitle: x\n---\n> - body",
            "*em* and **strong**",
            "\\*escaped\\* &amp; `code`",
            "`multi\nline`",
            "a  \nb\\\nc",
            "> - _nested_",
            "[a](b \"title\") and ![alt](img)",
            "[use][ref]\n\n[ref]: /dest 'title'",
            "<https://x.y> <a@b.c>",
            "[[Note|shown]] and ~~*old*~~ ==new==",
            "$$\nmath\n$$ and $x$",
            "- [x] task\n  - [ ] nested",
            "www.example.com/path, user@example.com",
            "| *head* | [[Note]] |\n| :--- | ---: |\n| a \\| b | `code` |",
            "use[^Note]\n\n[^note]: first\ncontinuation",
            "*a ~~b* c~~",
            "[foo]: /url\nbar\n===\n\n[foo]",
            "---\ntitle: x\n---\nbody",
            "---\nunterminated",
        ];

        for input in inputs {
            let document = parse(input);
            let mut previous_end = 0;

            if let Some(frontmatter) = document.frontmatter {
                assert_valid_span(input, frontmatter.span);
                previous_end = frontmatter.span.end;
            }

            assert_block_sequence(
                input,
                &document.children,
                span(previous_end, input.len() as u32),
            );

            for definition in document.definitions {
                assert_valid_span(input, definition.span);
                assert_span_within(definition.label, definition.span, input);
                assert_span_within(definition.dest, definition.span, input);
                if let Some(title) = definition.title {
                    assert_span_within(title, definition.span, input);
                }
            }

            for diagnostic in document.diagnostics {
                assert_valid_span(input, diagnostic.span);
            }
        }
    }

    fn assert_valid_span(input: &str, span: Span) {
        assert!(span.start <= span.end, "input: {input:?}, span: {span:?}");
        assert!(
            span.end as usize <= input.len(),
            "input: {input:?}, span: {span:?}"
        );
        assert!(
            input.is_char_boundary(span.start as usize),
            "input: {input:?}, span: {span:?}"
        );
        assert!(
            input.is_char_boundary(span.end as usize),
            "input: {input:?}, span: {span:?}"
        );
    }

    fn assert_span_within(span: Span, parent: Span, input: &str) {
        assert_valid_span(input, span);
        assert!(span.start >= parent.start, "input: {input:?}");
        assert!(span.end <= parent.end, "input: {input:?}");
    }

    fn assert_block_sequence(input: &str, blocks: &[Block], parent: Span) {
        let mut previous_end = parent.start;
        for block in blocks {
            let block_span = match block {
                Block::Paragraph { span, content } | Block::Heading { span, content, .. } => {
                    assert_inline_sequence(input, content, *span);
                    *span
                }
                Block::CodeBlock { span, literal, .. } => {
                    assert_fragment_sequence(input, literal, *span);
                    *span
                }
                Block::ThematicBreak { span } => *span,
                Block::BlockQuote { span, children } => {
                    assert_block_sequence(input, children, *span);
                    *span
                }
                Block::List {
                    span: list_span,
                    items,
                    ..
                } => {
                    let mut item_end = list_span.start;
                    for item in items {
                        assert_valid_span(input, item.span);
                        assert!(item.span.start >= item_end, "input: {input:?}");
                        assert!(item.span.start >= list_span.start, "input: {input:?}");
                        assert!(item.span.end <= list_span.end, "input: {input:?}");
                        assert_block_sequence(input, &item.children, item.span);
                        item_end = item.span.end;
                    }
                    *list_span
                }
                Block::Table {
                    span,
                    alignments,
                    head,
                    rows,
                } => {
                    assert_eq!(head.cells.len(), alignments.len(), "input: {input:?}");
                    assert_table_row(input, head, *span);
                    let mut row_end = head.span.end;
                    for row in rows {
                        assert!(row.span.start >= row_end, "input: {input:?}");
                        assert_eq!(row.cells.len(), alignments.len(), "input: {input:?}");
                        assert_table_row(input, row, *span);
                        row_end = row.span.end;
                    }
                    *span
                }
                Block::FootnoteDefinition {
                    span,
                    label,
                    children,
                } => {
                    assert_span_within(*label, *span, input);
                    assert_block_sequence(input, children, *span);
                    *span
                }
            };
            assert_valid_span(input, block_span);
            assert!(block_span.start >= previous_end, "input: {input:?}");
            assert!(block_span.start >= parent.start, "input: {input:?}");
            assert!(block_span.end <= parent.end, "input: {input:?}");
            previous_end = block_span.end;
        }
    }

    fn assert_table_row(input: &str, row: &TableRow, parent: Span) {
        assert_span_within(row.span, parent, input);
        let mut cell_end = row.span.start;
        for cell in &row.cells {
            assert_span_within(cell.span, row.span, input);
            assert!(cell.span.start >= cell_end, "input: {input:?}");
            assert_inline_sequence(input, &cell.content, cell.span);
            cell_end = cell.span.end;
        }
    }

    fn assert_fragment_sequence(input: &str, fragments: &[Span], parent: Span) {
        let mut previous_end = parent.start;
        for fragment in fragments {
            assert_valid_span(input, *fragment);
            assert!(fragment.start >= previous_end, "input: {input:?}");
            assert!(fragment.start >= parent.start, "input: {input:?}");
            assert!(fragment.end <= parent.end, "input: {input:?}");
            previous_end = fragment.end;
        }
    }

    fn assert_inline_sequence(input: &str, inlines: &[Inline], parent: Span) {
        let mut previous_end = parent.start;
        for inline in inlines {
            let (inline_span, children): (Span, &[Inline]) = match inline {
                Inline::Text { span }
                | Inline::Escaped { span }
                | Inline::CharacterReference { span, .. }
                | Inline::SoftBreak { span }
                | Inline::HardBreak { span } => (*span, &[]),
                Inline::Autolink { span, uri, .. } => {
                    assert_span_within(*uri, *span, input);
                    (*span, &[])
                }
                Inline::WikiLink {
                    span,
                    target,
                    label,
                } => {
                    assert_span_within(*target, *span, input);
                    if let Some(label) = label {
                        assert_span_within(*label, *span, input);
                    }
                    (*span, &[])
                }
                Inline::FootnoteReference { span, label } => {
                    assert_span_within(*label, *span, input);
                    (*span, &[])
                }
                Inline::CodeSpan { span, literal } | Inline::Math { span, literal, .. } => {
                    assert_fragment_sequence(input, literal, *span);
                    (*span, &[])
                }
                Inline::Emphasis { span, children }
                | Inline::Strong { span, children }
                | Inline::Strikethrough { span, children }
                | Inline::Highlight { span, children } => (*span, children),
                Inline::Link {
                    span,
                    target,
                    children,
                }
                | Inline::Image {
                    span,
                    target,
                    children,
                } => {
                    for target_span in [target.dest, target.title, target.label]
                        .into_iter()
                        .flatten()
                    {
                        assert_valid_span(input, target_span);
                    }
                    (*span, children)
                }
            };
            assert_valid_span(input, inline_span);
            assert!(inline_span.start >= previous_end, "input: {input:?}");
            assert!(inline_span.start >= parent.start, "input: {input:?}");
            assert!(inline_span.end <= parent.end, "input: {input:?}");
            assert_inline_sequence(input, children, inline_span);
            previous_end = inline_span.end;
        }
    }
}
