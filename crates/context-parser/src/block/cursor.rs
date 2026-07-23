use context_lexer::TokenKind;

use crate::{
    Span,
    lines::{Line, PosToken},
};

#[derive(Clone)]
pub(super) struct LineCursor<'a> {
    source: &'a str,
    line: &'a Line,
    token_index: usize,
    byte_offset: u32,
    column: u32,
}

impl<'a> LineCursor<'a> {
    pub(super) fn new(source: &'a str, line: &'a Line) -> Self {
        Self {
            source,
            line,
            token_index: 0,
            byte_offset: 0,
            column: 0,
        }
    }

    pub(super) fn column(&self) -> u32 {
        self.column
    }

    pub(super) fn position(&self) -> u32 {
        self.current_token()
            .map_or(self.line.content_end(), |token| {
                token.span.start + self.byte_offset
            })
    }

    pub(super) fn kind(&self) -> Option<TokenKind> {
        self.current_token().map(|token| token.kind)
    }

    pub(super) fn remaining_token_span(&self) -> Option<Span> {
        self.current_token().map(|token| Span {
            start: token.span.start + self.byte_offset,
            end: token.span.end,
        })
    }

    pub(super) fn remaining_token_text(&self) -> Option<&'a str> {
        self.remaining_token_span()
            .map(|span| span.slice(self.source))
    }

    pub(super) fn is_eol(&self) -> bool {
        self.current_token().is_none()
    }

    pub(super) fn is_blank(&self) -> bool {
        self.line.tokens[self.token_index..]
            .iter()
            .all(|token| token.kind == TokenKind::Whitespace)
    }

    pub(super) fn skip_indent_up_to(&mut self, maximum: u32) -> bool {
        let available = self.indent_columns();
        if available > maximum {
            return false;
        }
        self.consume_whitespace_columns(available)
    }

    pub(super) fn indent_columns(&self) -> u32 {
        let mut cursor = self.clone();
        let start = cursor.column;
        while cursor.consume_whitespace_character() {}
        cursor.column - start
    }

    pub(super) fn consume_whitespace_columns(&mut self, columns: u32) -> bool {
        let target = self.column.saturating_add(columns);
        while self.column < target {
            let before = self.column;
            if !self.consume_whitespace_character() {
                return false;
            }
            if self.column > target {
                // Consuming a partial tab would require a transformed source
                // fragment, so P2 stops at the whole-character boundary.
                self.restore_whitespace_character(before);
                return false;
            }
        }
        true
    }

    pub(super) fn consume_optional_whitespace_character(&mut self) {
        self.consume_whitespace_character();
    }

    pub(super) fn consume_gt(&mut self) -> bool {
        if self.kind() != Some(TokenKind::GtRun) {
            return false;
        }
        self.byte_offset += 1;
        self.column += 1;
        self.normalize_token_offset();
        true
    }

    pub(super) fn consume_token(&mut self) -> bool {
        let Some(span) = self.remaining_token_span() else {
            return false;
        };
        let text = span.slice(self.source);
        for character in text.chars() {
            self.column = next_column(self.column, character);
        }
        self.token_index += 1;
        self.byte_offset = 0;
        true
    }

    pub(super) fn view(&self) -> LineView {
        let mut tokens = self.line.tokens[self.token_index..].to_vec();
        if let Some(first) = tokens.first_mut() {
            first.span.start = first.span.start.saturating_add(self.byte_offset);
        }
        LineView {
            tokens,
            start: self.position(),
            end: self.line.content_end(),
            base_column: self.column,
        }
    }

    fn current_token(&self) -> Option<&PosToken> {
        self.line.tokens.get(self.token_index)
    }

    fn consume_whitespace_character(&mut self) -> bool {
        if self.kind() != Some(TokenKind::Whitespace) {
            return false;
        }
        let Some(text) = self.remaining_token_text() else {
            return false;
        };
        let Some(character) = text.chars().next() else {
            return false;
        };
        self.byte_offset += character.len_utf8() as u32;
        self.column = next_column(self.column, character);
        self.normalize_token_offset();
        true
    }

    fn restore_whitespace_character(&mut self, previous_column: u32) {
        if self.token_index == 0 && self.byte_offset == 0 {
            return;
        }
        if self.byte_offset == 0 {
            self.token_index -= 1;
            self.byte_offset = self.line.tokens[self.token_index].span.end
                - self.line.tokens[self.token_index].span.start;
        }
        let token = &self.line.tokens[self.token_index];
        let consumed =
            &self.source[token.span.start as usize..(token.span.start + self.byte_offset) as usize];
        if let Some(character) = consumed.chars().next_back() {
            self.byte_offset -= character.len_utf8() as u32;
            self.column = previous_column;
        }
    }

    fn normalize_token_offset(&mut self) {
        if self.current_token().is_some_and(|token| {
            self.byte_offset == token.span.end.saturating_sub(token.span.start)
        }) {
            self.token_index += 1;
            self.byte_offset = 0;
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct LineView {
    pub(super) tokens: Vec<PosToken>,
    pub(super) start: u32,
    pub(super) end: u32,
    pub(super) base_column: u32,
}

impl LineView {
    pub(super) fn blank(&self) -> bool {
        self.tokens
            .iter()
            .all(|token| token.kind == TokenKind::Whitespace)
    }

    pub(super) fn indent_columns(&self, source: &str) -> u32 {
        let Some(token) = self
            .tokens
            .first()
            .filter(|token| token.kind == TokenKind::Whitespace)
        else {
            return 0;
        };
        let end_column = token
            .span
            .slice(source)
            .chars()
            .fold(self.base_column, next_column);
        end_column - self.base_column
    }

    pub(super) fn first_content_token(&self) -> usize {
        usize::from(
            self.tokens
                .first()
                .is_some_and(|token| token.kind == TokenKind::Whitespace),
        )
    }

    pub(super) fn content_span(&self) -> Span {
        let start = self
            .tokens
            .first()
            .filter(|token| token.kind == TokenKind::Whitespace)
            .map_or(self.start, |token| token.span.end);
        Span {
            start,
            end: self.end,
        }
    }

    pub(super) fn strip_indent(&self, source: &str, columns: u32) -> Span {
        let Some(whitespace) = self
            .tokens
            .first()
            .filter(|token| token.kind == TokenKind::Whitespace)
        else {
            return Span {
                start: self.start,
                end: self.end,
            };
        };
        let mut absolute_column = self.base_column;
        let target = self.base_column.saturating_add(columns);
        let mut consumed_bytes = 0;
        for character in whitespace.span.slice(source).chars() {
            let next = next_column(absolute_column, character);
            if next > target {
                // Partial tab expansion remains deliberately out of scope.
                break;
            }
            absolute_column = next;
            consumed_bytes += character.len_utf8() as u32;
            if absolute_column == target {
                break;
            }
        }
        Span {
            start: whitespace.span.start + consumed_bytes,
            end: self.end,
        }
    }
}

fn next_column(column: u32, character: char) -> u32 {
    if character == '\t' {
        column + (4 - column % 4)
    } else {
        column + 1
    }
}
