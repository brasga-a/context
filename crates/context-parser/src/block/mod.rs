mod cursor;

use context_lexer::{FrontmatterAllowed, TokenKind, tokenize};

use self::cursor::{LineCursor, LineView};
use crate::{
    CodeBlockKind, Diagnostic, DiagnosticKind, FrontmatterBlock, HeadingKind, ListKind, Span,
    TableAlignment,
    ast::{
        RawBlock as Block, RawDocument as Document, RawListItem as ListItem,
        RawTableRow as TableRow,
    },
    lines::{Line, PosToken, collect_lines},
};

struct OpenParagraph {
    fragments: Vec<Span>,
}

struct OpenFence {
    marker: TokenKind,
    marker_len: u32,
    indent_cols: u32,
    opening_span: Span,
    block_start: u32,
    last_content_end: u32,
    info: Option<Span>,
    literal: Vec<Span>,
}

struct OpenIndentedCode {
    block_start: u32,
    last_content_end: u32,
    literal: Vec<Span>,
    pending_blank_lines: Vec<Span>,
}

enum OpenLeaf {
    Paragraph(OpenParagraph),
    Fence(OpenFence),
    Indented(OpenIndentedCode),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListFamily {
    Bullet(u8),
    Ordered(u8),
}

enum ContainerFrame {
    BlockQuote {
        start: u32,
        end: u32,
        children: Vec<Block>,
    },
    List {
        start: u32,
        end: u32,
        kind: ListKind,
        family: ListFamily,
        loose: bool,
        pending_blank_after_item: bool,
        items: Vec<ListItem>,
    },
    ListItem {
        start: u32,
        end: u32,
        content_column: u32,
        pending_blank: bool,
        had_content: bool,
        children: Vec<Block>,
    },
}

pub(crate) fn parse(input: &str) -> Document {
    let mut tokens = tokenize(input, FrontmatterAllowed::Yes).peekable();
    let mut frontmatter = None;
    let mut diagnostics = Vec::new();
    let mut start_offset = 0;

    if let Some(token) = tokens.peek()
        && let TokenKind::Frontmatter { terminated } = token.kind
    {
        let token_len = token.len;
        tokens.next();
        let span = Span {
            start: 0,
            end: token_len,
        };
        frontmatter = Some(FrontmatterBlock { span, terminated });
        start_offset = token_len;
        if !terminated {
            diagnostics.push(Diagnostic {
                span,
                kind: DiagnosticKind::UnterminatedFrontmatter,
            });
        }
    }

    let lines = collect_lines(input, tokens, start_offset);
    let children = extract_tables(input, parse_lines(input, &lines, &mut diagnostics));
    Document {
        frontmatter,
        children,
        diagnostics,
    }
}

fn extract_tables(source: &str, blocks: Vec<Block>) -> Vec<Block> {
    let mut transformed = Vec::with_capacity(blocks.len());
    for block in blocks {
        match block {
            Block::Paragraph { span, fragments } => {
                let Some((delimiter_index, alignments)) =
                    fragments.windows(2).enumerate().find_map(|(index, pair)| {
                        let header_cells = split_table_row(source, pair[0]);
                        let alignments = table_delimiter(source, pair[1])?;
                        (header_cells.len() == alignments.len()).then_some((index + 1, alignments))
                    })
                else {
                    transformed.push(Block::Paragraph { span, fragments });
                    continue;
                };

                let header_index = delimiter_index - 1;
                if header_index > 0 {
                    transformed.push(Block::Paragraph {
                        span: Span {
                            start: fragments[0].start,
                            end: fragments[header_index - 1].end,
                        },
                        fragments: fragments[..header_index].to_vec(),
                    });
                }
                let column_count = alignments.len();
                let head = normalized_table_row(source, fragments[header_index], column_count);
                let rows = fragments[delimiter_index + 1..]
                    .iter()
                    .map(|row| normalized_table_row(source, *row, column_count))
                    .collect::<Vec<_>>();
                let table_end = rows
                    .last()
                    .map_or(fragments[delimiter_index].end, |row| row.span.end);
                transformed.push(Block::Table {
                    span: Span {
                        start: fragments[header_index].start,
                        end: table_end,
                    },
                    alignments,
                    head,
                    rows,
                });
            }
            Block::BlockQuote { span, children } => transformed.push(Block::BlockQuote {
                span,
                children: extract_tables(source, children),
            }),
            Block::List {
                span,
                kind,
                tight,
                items,
            } => transformed.push(Block::List {
                span,
                kind,
                tight,
                items: items
                    .into_iter()
                    .map(|item| ListItem {
                        span: item.span,
                        task: item.task,
                        children: extract_tables(source, item.children),
                    })
                    .collect(),
            }),
            other => transformed.push(other),
        }
    }
    transformed
}

fn table_delimiter(source: &str, span: Span) -> Option<Vec<TableAlignment>> {
    let cells = split_table_row(source, span);
    if cells.is_empty() {
        return None;
    }
    cells
        .into_iter()
        .map(|cell| {
            let text = cell.slice(source).as_bytes();
            let left = text.first() == Some(&b':');
            let right = text.last() == Some(&b':');
            let dash_start = usize::from(left);
            let dash_end = text.len().saturating_sub(usize::from(right));
            if dash_start >= dash_end
                || !text[dash_start..dash_end].iter().all(|byte| *byte == b'-')
            {
                return None;
            }
            Some(match (left, right) {
                (false, false) => TableAlignment::None,
                (true, false) => TableAlignment::Left,
                (true, true) => TableAlignment::Center,
                (false, true) => TableAlignment::Right,
            })
        })
        .collect()
}

fn normalized_table_row(source: &str, span: Span, column_count: usize) -> TableRow {
    let mut cells = split_table_row(source, span);
    cells.truncate(column_count);
    cells.resize(
        column_count,
        Span {
            start: span.end,
            end: span.end,
        },
    );
    TableRow { span, cells }
}

fn split_table_row(source: &str, span: Span) -> Vec<Span> {
    let mut content = trim_table_whitespace(source, span);
    if source.as_bytes().get(content.start as usize) == Some(&b'|') {
        content.start += 1;
    }
    if content.start < content.end
        && source.as_bytes().get(content.end as usize - 1) == Some(&b'|')
        && !table_pipe_escaped(source, content.start, content.end - 1)
    {
        content.end -= 1;
    }

    let mut cells = Vec::new();
    let mut cell_start = content.start;
    let mut position = content.start;
    while position < content.end {
        if source.as_bytes()[position as usize] == b'|'
            && !table_pipe_escaped(source, content.start, position)
        {
            cells.push(trim_table_whitespace(
                source,
                Span {
                    start: cell_start,
                    end: position,
                },
            ));
            cell_start = position + 1;
        }
        position += 1;
    }
    cells.push(trim_table_whitespace(
        source,
        Span {
            start: cell_start,
            end: content.end,
        },
    ));
    cells
}

fn trim_table_whitespace(source: &str, mut span: Span) -> Span {
    while span.start < span.end && matches!(source.as_bytes()[span.start as usize], b' ' | b'\t') {
        span.start += 1;
    }
    while span.start < span.end && matches!(source.as_bytes()[span.end as usize - 1], b' ' | b'\t')
    {
        span.end -= 1;
    }
    span
}

fn table_pipe_escaped(source: &str, row_start: u32, pipe: u32) -> bool {
    source[row_start as usize..pipe as usize]
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\\')
        .count()
        % 2
        == 1
}

fn parse_lines(source: &str, lines: &[Line], diagnostics: &mut Vec<Diagnostic>) -> Vec<Block> {
    let mut root = Vec::new();
    let mut containers = Vec::new();
    let mut leaf = None;

    for line in lines {
        let mut cursor = LineCursor::new(source, line);
        let mut failure = None;

        for frame_index in 0..containers.len() {
            let mut loosen_parent_list = false;
            match &mut containers[frame_index] {
                ContainerFrame::BlockQuote { end, .. } => {
                    let mut candidate = cursor.clone();
                    if consume_blockquote_marker(&mut candidate).is_some() {
                        cursor = candidate;
                        *end = line.content_end();
                    } else {
                        failure = Some(frame_index);
                        break;
                    }
                }
                ContainerFrame::List { end, .. } => *end = line.content_end(),
                ContainerFrame::ListItem {
                    end,
                    content_column,
                    pending_blank,
                    had_content,
                    ..
                } => {
                    if cursor.is_blank() {
                        if !*had_content {
                            failure = Some(frame_index);
                            break;
                        }
                        // A blank line keeps the item open but contributes no
                        // content, so the item span must not extend to it.
                        *pending_blank = true;
                    } else {
                        let required = content_column.saturating_sub(cursor.column());
                        let mut candidate = cursor.clone();
                        if candidate.indent_columns() >= required
                            && candidate.consume_whitespace_columns(required)
                        {
                            if *pending_blank && *had_content {
                                loosen_parent_list = true;
                            }
                            *pending_blank = false;
                            *had_content = true;
                            *end = line.content_end();
                            cursor = candidate;
                        } else {
                            failure = Some(frame_index);
                            break;
                        }
                    }
                }
            }
            if loosen_parent_list {
                mark_parent_list_loose(&mut containers, frame_index);
            }
        }

        if let Some(frame_index) = failure {
            let view = cursor.view();
            let starts_sibling_item = matches!(
                containers.get(frame_index),
                Some(ContainerFrame::ListItem { .. })
            ) && parse_list_marker(source, &cursor).is_some();
            let lazy = matches!(leaf, Some(OpenLeaf::Paragraph(_)))
                && !view.blank()
                && !starts_sibling_item
                && !starts_interrupting_block(source, &cursor, &view);
            if lazy {
                if let Some(OpenLeaf::Paragraph(paragraph)) = leaf.as_mut() {
                    paragraph.fragments.push(view.content_span());
                }
                update_container_ends(&mut containers, line.content_end());
                continue;
            }
            close_leaf(&mut leaf, &mut containers, &mut root, diagnostics);
            close_containers_from(frame_index, &mut containers, &mut root);
        }

        if process_open_leaf(
            source,
            line,
            &cursor,
            &mut leaf,
            &mut containers,
            &mut root,
            diagnostics,
        ) {
            continue;
        }

        loop {
            let mut quote_cursor = cursor.clone();
            if let Some(marker_start) = consume_blockquote_marker(&mut quote_cursor) {
                close_leaf(&mut leaf, &mut containers, &mut root, diagnostics);
                containers.push(ContainerFrame::BlockQuote {
                    start: marker_start,
                    end: line.content_end(),
                    children: Vec::new(),
                });
                cursor = quote_cursor;
                continue;
            }

            let view = cursor.view();
            if view.blank() {
                break;
            }
            if paragraph_is_open(&leaf) && setext_level(source, &view).is_some()
                || thematic_break(source, &view).is_some()
            {
                // A thematic break at list level ends the list: without this,
                // the break is appended to root while the List frame stays
                // open, so `- foo\n***\n- bar` would merge into one list
                // emitted AFTER the break. Harmless in the setext arm (the
                // top frame is then a ListItem, making this a no-op).
                close_dangling_list(&mut containers, &mut root);
                break;
            }

            let Some(marker) = parse_list_marker(source, &cursor) else {
                close_dangling_list(&mut containers, &mut root);
                break;
            };
            let may_interrupt = !paragraph_is_open(&leaf)
                || (!marker.empty
                    && match marker.kind {
                        ListKind::Bullet { .. } => true,
                        ListKind::Ordered { start, .. } => start == 1,
                    });
            if !may_interrupt {
                break;
            }

            close_leaf(&mut leaf, &mut containers, &mut root, diagnostics);
            prepare_list_for_item(
                marker.family,
                marker.kind,
                marker.marker_start,
                line.content_end(),
                &mut containers,
                &mut root,
            );
            containers.push(ContainerFrame::ListItem {
                start: marker.marker_start,
                end: line.content_end(),
                content_column: marker.content_column,
                pending_blank: false,
                had_content: !marker.empty,
                children: Vec::new(),
            });
            cursor = marker.cursor_after;
        }

        let view = cursor.view();
        process_new_leaf(
            source,
            line,
            &view,
            &mut leaf,
            &mut containers,
            &mut root,
            diagnostics,
        );
    }

    close_leaf(&mut leaf, &mut containers, &mut root, diagnostics);
    close_containers_from(0, &mut containers, &mut root);
    root
}

fn process_open_leaf(
    source: &str,
    line: &Line,
    cursor: &LineCursor<'_>,
    leaf: &mut Option<OpenLeaf>,
    containers: &mut [ContainerFrame],
    root: &mut Vec<Block>,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let view = cursor.view();
    match leaf {
        Some(OpenLeaf::Fence(open)) => {
            if is_fence_closer(source, &view, open.marker, open.marker_len) {
                let open = match leaf.take() {
                    Some(OpenLeaf::Fence(open)) => open,
                    _ => return true,
                };
                append_block(
                    containers,
                    root,
                    Block::CodeBlock {
                        span: Span {
                            start: open.block_start,
                            end: view.end,
                        },
                        kind: CodeBlockKind::Fenced { info: open.info },
                        literal: open.literal,
                    },
                );
            } else {
                open.literal
                    .push(view.strip_indent(source, open.indent_cols));
                open.last_content_end = view.end;
            }
            update_container_ends(containers, line.content_end());
            true
        }
        Some(OpenLeaf::Indented(open)) => {
            if view.blank() {
                open.pending_blank_lines.push(view.strip_indent(source, 4));
                update_container_ends(containers, line.content_end());
                true
            } else if view.indent_columns(source) >= 4 {
                open.literal.append(&mut open.pending_blank_lines);
                open.literal.push(view.strip_indent(source, 4));
                open.last_content_end = view.end;
                update_container_ends(containers, line.content_end());
                true
            } else {
                close_leaf(leaf, containers, root, diagnostics);
                false
            }
        }
        _ => false,
    }
}

fn process_new_leaf(
    source: &str,
    line: &Line,
    view: &LineView,
    leaf: &mut Option<OpenLeaf>,
    containers: &mut [ContainerFrame],
    root: &mut Vec<Block>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if view.blank() {
        close_leaf(leaf, containers, root, diagnostics);
        mark_blank_in_item(containers);
        return;
    }

    if let Some(block) = atx_heading(source, view) {
        close_leaf(leaf, containers, root, diagnostics);
        append_block(containers, root, block);
    } else if let Some(open) = fenced_code_opening(source, view) {
        close_leaf(leaf, containers, root, diagnostics);
        *leaf = Some(OpenLeaf::Fence(open));
    } else if paragraph_is_open(leaf)
        && let Some(level) = setext_level(source, view)
    {
        if let Some(OpenLeaf::Paragraph(paragraph)) = leaf.take()
            && let Some(first) = paragraph.fragments.first()
        {
            append_block(
                containers,
                root,
                Block::Heading {
                    span: Span {
                        start: first.start,
                        end: view.end,
                    },
                    level,
                    kind: HeadingKind::Setext,
                    fragments: paragraph.fragments,
                },
            );
        }
    } else if let Some(span) = thematic_break(source, view) {
        close_leaf(leaf, containers, root, diagnostics);
        append_block(containers, root, Block::ThematicBreak { span });
    } else if !paragraph_is_open(leaf) && view.indent_columns(source) >= 4 {
        let literal = view.strip_indent(source, 4);
        *leaf = Some(OpenLeaf::Indented(OpenIndentedCode {
            block_start: literal.start,
            last_content_end: view.end,
            literal: vec![literal],
            pending_blank_lines: Vec::new(),
        }));
    } else {
        match leaf {
            Some(OpenLeaf::Paragraph(paragraph)) => paragraph.fragments.push(view.content_span()),
            _ => {
                close_leaf(leaf, containers, root, diagnostics);
                *leaf = Some(OpenLeaf::Paragraph(OpenParagraph {
                    fragments: vec![view.content_span()],
                }));
            }
        }
    }
    update_container_ends(containers, line.content_end());
    mark_item_content(containers);
}

fn consume_blockquote_marker(cursor: &mut LineCursor<'_>) -> Option<u32> {
    if !cursor.skip_indent_up_to(3) {
        return None;
    }
    let start = cursor.position();
    if !cursor.consume_gt() {
        return None;
    }
    cursor.consume_optional_whitespace_character();
    Some(start)
}

struct ParsedListMarker<'a> {
    family: ListFamily,
    kind: ListKind,
    marker_start: u32,
    content_column: u32,
    empty: bool,
    cursor_after: LineCursor<'a>,
}

fn parse_list_marker<'a>(source: &str, cursor: &LineCursor<'a>) -> Option<ParsedListMarker<'a>> {
    let mut cursor = cursor.clone();
    if !cursor.skip_indent_up_to(3) {
        return None;
    }
    let marker_start = cursor.position();
    let marker_text = cursor.remaining_token_text()?;
    let (family, kind) = match cursor.kind()? {
        TokenKind::DashRun if marker_text == "-" => {
            (ListFamily::Bullet(b'-'), ListKind::Bullet { marker: b'-' })
        }
        TokenKind::StarRun if marker_text == "*" => {
            (ListFamily::Bullet(b'*'), ListKind::Bullet { marker: b'*' })
        }
        TokenKind::Text if marker_text == "+" => {
            (ListFamily::Bullet(b'+'), ListKind::Bullet { marker: b'+' })
        }
        TokenKind::Text => {
            let (number, delimiter) = parse_ordered_marker(marker_text)?;
            (
                ListFamily::Ordered(delimiter),
                ListKind::Ordered {
                    start: number,
                    delimiter,
                },
            )
        }
        _ => return None,
    };
    cursor.consume_token();
    if !cursor.is_eol() && cursor.kind() != Some(TokenKind::Whitespace) {
        return None;
    }

    let marker_end_column = cursor.column();
    let empty = cursor.is_blank();
    let available = cursor.indent_columns();
    let content_column = if empty || available == 0 {
        marker_end_column + 1
    } else if available <= 4 {
        cursor.consume_whitespace_columns(available);
        cursor.column()
    } else {
        cursor.consume_whitespace_columns(1);
        marker_end_column + 1
    };

    let _ = source;
    Some(ParsedListMarker {
        family,
        kind,
        marker_start,
        content_column,
        empty,
        cursor_after: cursor,
    })
}

fn parse_ordered_marker(text: &str) -> Option<(u32, u8)> {
    if !text.is_ascii() || !(2..=10).contains(&text.len()) {
        return None;
    }
    let (digits, delimiter) = text.split_at(text.len() - 1);
    let delimiter = delimiter.as_bytes()[0];
    if !matches!(delimiter, b'.' | b')')
        || digits.is_empty()
        || digits.len() > 9
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some((digits.parse().ok()?, delimiter))
}

fn prepare_list_for_item(
    family: ListFamily,
    kind: ListKind,
    start: u32,
    end: u32,
    containers: &mut Vec<ContainerFrame>,
    root: &mut Vec<Block>,
) {
    let reuse = matches!(containers.last(), Some(ContainerFrame::List { family: open, .. }) if *open == family);
    if !reuse {
        close_dangling_list(containers, root);
        containers.push(ContainerFrame::List {
            start,
            end,
            kind,
            family,
            loose: false,
            pending_blank_after_item: false,
            items: Vec::new(),
        });
    } else if let Some(ContainerFrame::List {
        loose,
        pending_blank_after_item,
        end: list_end,
        ..
    }) = containers.last_mut()
    {
        if *pending_blank_after_item {
            *loose = true;
            *pending_blank_after_item = false;
        }
        *list_end = end;
    }
}

fn close_dangling_list(containers: &mut Vec<ContainerFrame>, root: &mut Vec<Block>) {
    if matches!(containers.last(), Some(ContainerFrame::List { .. })) {
        close_containers_from(containers.len() - 1, containers, root);
    }
}

fn close_leaf(
    leaf: &mut Option<OpenLeaf>,
    containers: &mut [ContainerFrame],
    root: &mut Vec<Block>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(open) = leaf.take() else {
        return;
    };
    let block = match open {
        OpenLeaf::Paragraph(paragraph) => {
            let Some(first) = paragraph.fragments.first() else {
                return;
            };
            let Some(last) = paragraph.fragments.last() else {
                return;
            };
            Block::Paragraph {
                span: Span {
                    start: first.start,
                    end: last.end,
                },
                fragments: paragraph.fragments,
            }
        }
        OpenLeaf::Fence(fence) => {
            diagnostics.push(Diagnostic {
                span: fence.opening_span,
                kind: DiagnosticKind::UnclosedCodeFence,
            });
            Block::CodeBlock {
                span: Span {
                    start: fence.block_start,
                    end: fence.last_content_end,
                },
                kind: CodeBlockKind::Fenced { info: fence.info },
                literal: fence.literal,
            }
        }
        OpenLeaf::Indented(code) => Block::CodeBlock {
            span: Span {
                start: code.block_start,
                end: code.last_content_end,
            },
            kind: CodeBlockKind::Indented,
            literal: code.literal,
        },
    };
    append_block(containers, root, block);
}

fn close_containers_from(
    depth: usize,
    containers: &mut Vec<ContainerFrame>,
    root: &mut Vec<Block>,
) {
    while containers.len() > depth {
        let Some(frame) = containers.pop() else {
            break;
        };
        match frame {
            ContainerFrame::BlockQuote {
                start,
                end,
                children,
            } => append_block(
                containers,
                root,
                Block::BlockQuote {
                    span: Span { start, end },
                    children,
                },
            ),
            ContainerFrame::ListItem {
                start,
                end,
                pending_blank,
                had_content,
                children,
                ..
            } => {
                if let Some(ContainerFrame::List {
                    items,
                    pending_blank_after_item,
                    end: list_end,
                    ..
                }) = containers.last_mut()
                {
                    items.push(ListItem {
                        span: Span { start, end },
                        task: None,
                        children,
                    });
                    *pending_blank_after_item = pending_blank && had_content;
                    *list_end = end;
                }
            }
            ContainerFrame::List {
                start,
                end,
                kind,
                loose,
                items,
                ..
            } => {
                let end = items.last().map_or(end, |item| item.span.end);
                append_block(
                    containers,
                    root,
                    Block::List {
                        span: Span { start, end },
                        kind,
                        tight: !loose,
                        items,
                    },
                )
            }
        }
    }
}

fn append_block(containers: &mut [ContainerFrame], root: &mut Vec<Block>, block: Block) {
    if let Some(parent) = containers.iter_mut().rev().find(|frame| {
        matches!(
            frame,
            ContainerFrame::BlockQuote { .. } | ContainerFrame::ListItem { .. }
        )
    }) {
        match parent {
            ContainerFrame::BlockQuote { children, .. }
            | ContainerFrame::ListItem { children, .. } => children.push(block),
            ContainerFrame::List { .. } => {}
        }
    } else {
        root.push(block);
    }
}

fn mark_parent_list_loose(containers: &mut [ContainerFrame], item_index: usize) {
    if let Some(ContainerFrame::List { loose, .. }) = containers[..item_index]
        .iter_mut()
        .rev()
        .find(|frame| matches!(frame, ContainerFrame::List { .. }))
    {
        *loose = true;
    }
}

fn mark_blank_in_item(containers: &mut [ContainerFrame]) {
    if let Some(ContainerFrame::ListItem { pending_blank, .. }) = containers
        .iter_mut()
        .rev()
        .find(|frame| matches!(frame, ContainerFrame::ListItem { .. }))
    {
        *pending_blank = true;
    }
}

fn mark_item_content(containers: &mut [ContainerFrame]) {
    if let Some(ContainerFrame::ListItem { had_content, .. }) = containers
        .iter_mut()
        .rev()
        .find(|frame| matches!(frame, ContainerFrame::ListItem { .. }))
    {
        *had_content = true;
    }
}

fn update_container_ends(containers: &mut [ContainerFrame], end: u32) {
    for frame in containers {
        match frame {
            ContainerFrame::BlockQuote { end: frame_end, .. }
            | ContainerFrame::List { end: frame_end, .. }
            | ContainerFrame::ListItem { end: frame_end, .. } => *frame_end = end,
        }
    }
}

fn paragraph_is_open(leaf: &Option<OpenLeaf>) -> bool {
    matches!(leaf, Some(OpenLeaf::Paragraph(_)))
}

fn starts_interrupting_block(source: &str, cursor: &LineCursor<'_>, view: &LineView) -> bool {
    // A setext underline is deliberately absent here: it cannot start a block
    // on its own, so it can never interrupt a lazy paragraph continuation
    // (CommonMark: "the setext heading underline cannot be a lazy
    // continuation line" — it becomes plain paragraph text instead).
    atx_heading(source, view).is_some()
        || fenced_code_opening(source, view).is_some()
        || thematic_break(source, view).is_some()
        || consume_blockquote_marker(&mut cursor.clone()).is_some()
        || parse_list_marker(source, cursor).is_some_and(|marker| {
            !marker.empty
                && match marker.kind {
                    ListKind::Bullet { .. } => true,
                    ListKind::Ordered { start, .. } => start == 1,
                }
        })
}

fn atx_heading(source: &str, line: &LineView) -> Option<Block> {
    if line.indent_columns(source) > 3 {
        return None;
    }
    let marker_index = line.first_content_token();
    let marker = line.tokens.get(marker_index)?;
    if marker.kind != TokenKind::HashRun {
        return None;
    }
    let level = token_len(marker);
    if !(1..=6).contains(&level)
        || line
            .tokens
            .get(marker_index + 1)
            .is_some_and(|token| token.kind != TokenKind::Whitespace)
    {
        return None;
    }
    let raw_start = marker_index + 1;
    let mut start = raw_start;
    while line
        .tokens
        .get(start)
        .is_some_and(|token| token.kind == TokenKind::Whitespace)
    {
        start += 1;
    }
    let mut end = line.tokens.len();
    while end > start && line.tokens[end - 1].kind == TokenKind::Whitespace {
        end -= 1;
    }
    if end > raw_start
        && line.tokens[end - 1].kind == TokenKind::HashRun
        && end - 1 > raw_start
        && line.tokens[end - 2].kind == TokenKind::Whitespace
    {
        end -= 1;
        while end > raw_start && line.tokens[end - 1].kind == TokenKind::Whitespace {
            end -= 1;
        }
    }
    let fragments = (start < end)
        .then(|| Span {
            start: line.tokens[start].span.start,
            end: line.tokens[end - 1].span.end,
        })
        .into_iter()
        .collect();
    Some(Block::Heading {
        span: Span {
            start: marker.span.start,
            end: line.end,
        },
        level: level as u8,
        kind: HeadingKind::Atx,
        fragments,
    })
}

fn fenced_code_opening(source: &str, line: &LineView) -> Option<OpenFence> {
    let indent_cols = line.indent_columns(source);
    if indent_cols > 3 {
        return None;
    }
    let marker_index = line.first_content_token();
    let marker = line.tokens.get(marker_index)?;
    if !matches!(marker.kind, TokenKind::BacktickRun | TokenKind::TildeRun) || token_len(marker) < 3
    {
        return None;
    }
    let info = trimmed_token_span(&line.tokens, marker_index + 1, line.tokens.len());
    if marker.kind == TokenKind::BacktickRun
        && info.is_some_and(|span| span.slice(source).contains('`'))
    {
        return None;
    }
    Some(OpenFence {
        marker: marker.kind,
        marker_len: token_len(marker),
        indent_cols,
        opening_span: line.content_span(),
        block_start: marker.span.start,
        last_content_end: line.end,
        info,
        literal: Vec::new(),
    })
}

fn is_fence_closer(
    source: &str,
    line: &LineView,
    marker_kind: TokenKind,
    opening_len: u32,
) -> bool {
    if line.indent_columns(source) > 3 {
        return false;
    }
    let marker_index = line.first_content_token();
    let Some(marker) = line.tokens.get(marker_index) else {
        return false;
    };
    marker.kind == marker_kind
        && token_len(marker) >= opening_len
        && line.tokens[marker_index + 1..]
            .iter()
            .all(|token| token.kind == TokenKind::Whitespace)
}

fn setext_level(source: &str, line: &LineView) -> Option<u8> {
    if line.indent_columns(source) > 3 {
        return None;
    }
    let marker_index = line.first_content_token();
    let marker = line.tokens.get(marker_index)?;
    let level = match marker.kind {
        TokenKind::EqualsRun => 1,
        TokenKind::DashRun => 2,
        _ => return None,
    };
    line.tokens[marker_index + 1..]
        .iter()
        .all(|token| token.kind == TokenKind::Whitespace)
        .then_some(level)
}

fn thematic_break(source: &str, line: &LineView) -> Option<Span> {
    if line.indent_columns(source) > 3 {
        return None;
    }
    let marker_index = line.first_content_token();
    let marker_kind = line.tokens.get(marker_index)?.kind;
    if !matches!(
        marker_kind,
        TokenKind::DashRun | TokenKind::StarRun | TokenKind::UnderscoreRun
    ) {
        return None;
    }
    let mut count = 0;
    for token in &line.tokens[marker_index..] {
        if token.kind == marker_kind {
            count += token_len(token);
        } else if token.kind != TokenKind::Whitespace {
            return None;
        }
    }
    (count >= 3).then_some(line.content_span())
}

fn trimmed_token_span(tokens: &[PosToken], mut start: usize, mut end: usize) -> Option<Span> {
    while start < end && tokens[start].kind == TokenKind::Whitespace {
        start += 1;
    }
    while end > start && tokens[end - 1].kind == TokenKind::Whitespace {
        end -= 1;
    }
    (start < end).then(|| Span {
        start: tokens[start].span.start,
        end: tokens[end - 1].span.end,
    })
}

fn token_len(token: &PosToken) -> u32 {
    token.span.end - token.span.start
}
