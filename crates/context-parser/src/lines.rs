use context_lexer::{Token, TokenKind};

use crate::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PosToken {
    pub(crate) kind: TokenKind,
    pub(crate) span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Line {
    pub(crate) tokens: Vec<PosToken>,
    pub(crate) span: Span,
    pub(crate) indent_cols: u32,
    pub(crate) blank: bool,
}

impl Line {
    pub(crate) fn content_end(&self) -> u32 {
        self.tokens
            .last()
            .map_or(self.span.start, |token| token.span.end)
    }
}

pub(crate) fn collect_lines(
    source: &str,
    tokens: impl IntoIterator<Item = Token>,
    start_offset: u32,
) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut line_tokens = Vec::new();
    let mut offset = start_offset;
    let mut line_start = start_offset;

    for token in tokens {
        let token_end = offset.saturating_add(token.len);
        let positioned = PosToken {
            kind: token.kind,
            span: Span {
                start: offset,
                end: token_end,
            },
        };
        offset = token_end;

        if token.kind == TokenKind::Newline {
            lines.push(finish_line(source, line_start, offset, &mut line_tokens));
            line_start = offset;
        } else {
            line_tokens.push(positioned);
        }
    }

    if !line_tokens.is_empty() {
        lines.push(finish_line(source, line_start, offset, &mut line_tokens));
    }

    lines
}

fn finish_line(source: &str, start: u32, end: u32, tokens: &mut Vec<PosToken>) -> Line {
    let tokens = std::mem::take(tokens);
    let blank = tokens
        .iter()
        .all(|token| token.kind == TokenKind::Whitespace);
    let indent_cols = tokens
        .first()
        .filter(|token| token.kind == TokenKind::Whitespace)
        .map_or(0, |token| indentation_columns(token.span.slice(source)));

    Line {
        tokens,
        span: Span { start, end },
        indent_cols,
        blank,
    }
}

fn indentation_columns(whitespace: &str) -> u32 {
    whitespace.chars().fold(0, |column, character| {
        if character == '\t' {
            column + (4 - column % 4)
        } else {
            column + 1
        }
    })
}

#[cfg(test)]
mod tests {
    use context_lexer::{FrontmatterAllowed, tokenize};

    use super::*;

    #[test]
    fn indentation_is_measured_in_columns_with_four_column_tab_stops() {
        let input = " \t x\n\t\tx";
        let lines = collect_lines(input, tokenize(input, FrontmatterAllowed::No), 0);

        assert_eq!(lines[0].indent_cols, 5);
        assert_eq!(lines[1].indent_cols, 8);
    }
}
