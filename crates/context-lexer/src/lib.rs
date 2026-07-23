//! Low-level, streaming lexer for Context.
//!
//! This lexer is deliberately "dumb": it turns raw text into runs of a few
//! primitive kinds (punctuation runs, text, whitespace, newlines) without deciding
//! whether a run of `#` is a heading, whether indentation matters, or any
//! other Markdown-block semantics. Those decisions belong to a block-parser
//! layer built on top of this token stream. Frontmatter is the one
//! exception: a `---` fence at the very start of the document is
//! unambiguous without any surrounding context, so it is recognized here.

use std::str::Chars;

use TokenKind::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub len: u32,
}

impl Token {
    pub fn new(kind: TokenKind, len: u32) -> Self {
        Self { kind, len }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// A contiguous run of `#` characters. `Token::len` is the number of
    /// hashes (and bytes, since `#` is ASCII). Whether this is a heading is
    /// decided by a later parsing layer, not the lexer.
    HashRun,
    /// A contiguous run of `-` characters. `Token::len` is the number of
    /// dashes (and bytes, since `-` is ASCII). Interpretation is deferred to
    /// a later parsing layer.
    DashRun,
    /// A contiguous run of `*` characters. `Token::len` is the number of
    /// stars (and bytes, since `*` is ASCII). Interpretation is deferred to
    /// a later parsing layer.
    StarRun,
    /// A contiguous run of `_` characters. `Token::len` is the number of
    /// underscores (and bytes, since `_` is ASCII). Interpretation is
    /// deferred to a later parsing layer.
    UnderscoreRun,
    /// A contiguous run of `=` characters. `Token::len` is the number of
    /// equals signs (and bytes, since `=` is ASCII). Interpretation is
    /// deferred to a later parsing layer.
    EqualsRun,
    /// A contiguous run of backtick characters. `Token::len` is the number
    /// of backticks (and bytes, since backticks are ASCII). Interpretation is
    /// deferred to a later parsing layer.
    BacktickRun,
    /// A contiguous run of `~` characters. `Token::len` is the number of
    /// tildes (and bytes, since `~` is ASCII). Interpretation is deferred to
    /// a later parsing layer.
    TildeRun,
    /// A contiguous run of `>` characters. `Token::len` is the number of
    /// greater-than signs (and bytes, since `>` is ASCII). Interpretation is
    /// deferred to a later parsing layer.
    GtRun,
    /// A contiguous run of any character other than punctuation run
    /// characters, horizontal whitespace, or a line ending.
    Text,
    /// A contiguous run of horizontal whitespace (spaces/tabs).
    Whitespace,
    /// A single line ending: `\n`, `\r`, or `\r\n` (`len == 2` for CRLF).
    Newline,
    Frontmatter {
        terminated: bool,
    },
    Eof,
}

fn fence_line_len(input: &str) -> Option<usize> {
    let rest = input.strip_prefix("---")?;
    if rest.starts_with('-') {
        return None;
    }

    let whitespace_len = rest
        .bytes()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count();
    let ending = &rest[whitespace_len..];
    let ending_len = if ending.starts_with("\r\n") {
        2
    } else if ending.starts_with('\r') || ending.starts_with('\n') {
        1
    } else if ending.is_empty() {
        0
    } else {
        return None;
    };

    Some(3 + whitespace_len + ending_len)
}

fn next_line_len(input: &str) -> usize {
    match memchr::memchr2(b'\r', b'\n', input.as_bytes()) {
        Some(index)
            if input.as_bytes()[index] == b'\r'
                && input.as_bytes().get(index + 1) == Some(&b'\n') =>
        {
            index + 2
        }
        Some(index) => index + 1,
        None => input.len(),
    }
}

pub fn tokenize(
    input: &str,
    frontmatter_allowed: FrontmatterAllowed,
) -> impl Iterator<Item = Token> + '_ {
    let mut cursor = Cursor::new(input, frontmatter_allowed);
    std::iter::from_fn(move || {
        let token = cursor.advance_token();
        (token.kind != Eof).then_some(token)
    })
}

pub fn is_horizontal_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t')
}

fn is_line_ending(c: char) -> bool {
    matches!(c, '\r' | '\n')
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontmatterAllowed {
    Yes,
    No,
}

pub struct Cursor<'a> {
    len_remaining: usize,
    chars: Chars<'a>,
    frontmatter_allowed: FrontmatterAllowed,
    at_document_start: bool,
}

const EOF_CHAR: char = '\0';

impl<'a> Cursor<'a> {
    pub fn new(input: &'a str, frontmatter_allowed: FrontmatterAllowed) -> Self {
        Self {
            len_remaining: input.len(),
            chars: input.chars(),
            frontmatter_allowed,
            at_document_start: true,
        }
    }

    pub fn as_str(&self) -> &'a str {
        self.chars.as_str()
    }

    pub fn first(&self) -> char {
        self.chars.clone().next().unwrap_or(EOF_CHAR)
    }

    pub(crate) fn is_eof(&self) -> bool {
        self.chars.as_str().is_empty()
    }

    pub(crate) fn pos_within_token(&self) -> u32 {
        (self.len_remaining - self.chars.as_str().len()) as u32
    }

    pub(crate) fn reset_pos_within_token(&mut self) {
        self.len_remaining = self.chars.as_str().len();
    }

    pub(crate) fn bump(&mut self) -> Option<char> {
        self.chars.next()
    }

    pub(crate) fn bump_bytes(&mut self, n: usize) {
        self.chars = self.as_str()[n..].chars();
    }

    pub(crate) fn eat_while(&mut self, mut predicate: impl FnMut(char) -> bool) {
        while !self.is_eof() && predicate(self.first()) {
            self.bump();
        }
    }

    pub fn advance_token(&mut self) -> Token {
        if self.at_document_start
            && matches!(self.frontmatter_allowed, FrontmatterAllowed::Yes)
            && let Some(terminated) = self.frontmatter()
        {
            self.at_document_start = false;
            return self.finish_token(Frontmatter { terminated });
        }

        let Some(first_char) = self.bump() else {
            return Token::new(Eof, 0);
        };

        let token_kind = match first_char {
            '\r' => {
                if self.first() == '\n' {
                    self.bump();
                }
                Newline
            }
            '\n' => Newline,
            c if is_horizontal_whitespace(c) => self.whitespace(),
            '#' => self.hash_run(),
            '-' => self.dash_run(),
            '*' => self.star_run(),
            '_' => self.underscore_run(),
            '=' => self.equals_run(),
            '`' => self.backtick_run(),
            '~' => self.tilde_run(),
            '>' => self.gt_run(),
            _ => self.text(),
        };

        self.at_document_start = false;
        self.finish_token(token_kind)
    }

    fn finish_token(&mut self, kind: TokenKind) -> Token {
        let token = Token::new(kind, self.pos_within_token());
        self.reset_pos_within_token();
        token
    }

    fn whitespace(&mut self) -> TokenKind {
        self.eat_while(is_horizontal_whitespace);
        Whitespace
    }

    fn hash_run(&mut self) -> TokenKind {
        self.eat_while(|c| c == '#');
        HashRun
    }

    fn dash_run(&mut self) -> TokenKind {
        self.eat_while(|c| c == '-');
        DashRun
    }

    fn star_run(&mut self) -> TokenKind {
        self.eat_while(|c| c == '*');
        StarRun
    }

    fn underscore_run(&mut self) -> TokenKind {
        self.eat_while(|c| c == '_');
        UnderscoreRun
    }

    fn equals_run(&mut self) -> TokenKind {
        self.eat_while(|c| c == '=');
        EqualsRun
    }

    fn backtick_run(&mut self) -> TokenKind {
        self.eat_while(|c| c == '`');
        BacktickRun
    }

    fn tilde_run(&mut self) -> TokenKind {
        self.eat_while(|c| c == '~');
        TildeRun
    }

    fn gt_run(&mut self) -> TokenKind {
        self.eat_while(|c| c == '>');
        GtRun
    }

    fn text(&mut self) -> TokenKind {
        self.eat_while(|c| {
            !matches!(c, '#' | '-' | '*' | '_' | '=' | '`' | '~' | '>')
                && !is_horizontal_whitespace(c)
                && !is_line_ending(c)
        });
        Text
    }

    fn frontmatter(&mut self) -> Option<bool> {
        let input = self.as_str();
        let bom_len = if input.starts_with('\u{feff}') {
            '\u{feff}'.len_utf8()
        } else {
            0
        };
        let opening_len = fence_line_len(&input[bom_len..])?;
        let mut consumed = bom_len + opening_len;
        let mut terminated = false;

        while consumed < input.len() {
            let line_len = next_line_len(&input[consumed..]);
            let line = &input[consumed..consumed + line_len];
            consumed += line_len;

            if fence_line_len(line) == Some(line_len) {
                terminated = true;
                break;
            }
        }

        self.bump_bytes(consumed);
        Some(terminated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(input: &str, allowed: FrontmatterAllowed) -> Vec<Token> {
        tokenize(input, allowed).collect()
    }

    fn assert_run_lengths(character: char, kind: TokenKind) {
        for count in 1..=4 {
            let input = character.to_string().repeat(count);
            assert_eq!(
                tokens(&input, FrontmatterAllowed::No),
                vec![Token::new(kind, count as u32)],
                "character = {character:?}, count = {count}"
            );
        }
    }

    fn assert_glued_text(input: &str, kind: TokenKind, trailing_text: &str) {
        assert_eq!(
            tokens(input, FrontmatterAllowed::No),
            vec![
                Token::new(Text, "text".len() as u32),
                Token::new(kind, 1),
                Token::new(Text, trailing_text.len() as u32),
            ]
        );
    }

    #[test]
    fn hash_run_counts_from_one_to_seven() {
        for count in 1..=7 {
            let input = "#".repeat(count);
            let toks = tokens(&input, FrontmatterAllowed::No);
            assert_eq!(
                toks,
                vec![Token::new(HashRun, count as u32)],
                "count = {count}"
            );
        }
    }

    #[test]
    fn hash_followed_by_word_is_hashrun_then_text() {
        let toks = tokens("#hashtag", FrontmatterAllowed::No);
        assert_eq!(
            toks,
            vec![
                Token::new(HashRun, 1),
                Token::new(Text, "hashtag".len() as u32)
            ]
        );
    }

    #[test]
    fn hash_inside_word_splits_into_separate_tokens() {
        let toks = tokens("text#hash", FrontmatterAllowed::No);
        assert_eq!(
            toks,
            vec![
                Token::new(Text, "text".len() as u32),
                Token::new(HashRun, 1),
                Token::new(Text, "hash".len() as u32),
            ]
        );
    }

    #[test]
    fn hash_after_indentation_has_no_special_meaning() {
        // The lexer no longer tracks indentation or heading validity: this
        // is just whitespace, then a hash run, then whitespace, then text.
        let toks = tokens("    ### heading", FrontmatterAllowed::No);
        assert_eq!(
            toks,
            vec![
                Token::new(Whitespace, 4),
                Token::new(HashRun, 3),
                Token::new(Whitespace, 1),
                Token::new(Text, "heading".len() as u32),
            ]
        );
    }

    #[test]
    fn dash_run_counts_from_one_to_four() {
        assert_run_lengths('-', DashRun);
    }

    #[test]
    fn dash_inside_text_splits_into_separate_tokens() {
        assert_glued_text("text-dash", DashRun, "dash");
    }

    #[test]
    fn star_run_counts_from_one_to_four() {
        assert_run_lengths('*', StarRun);
    }

    #[test]
    fn star_inside_text_splits_into_separate_tokens() {
        assert_glued_text("text*star", StarRun, "star");
    }

    #[test]
    fn underscore_run_counts_from_one_to_four() {
        assert_run_lengths('_', UnderscoreRun);
    }

    #[test]
    fn underscore_inside_text_splits_into_separate_tokens() {
        assert_glued_text("text_under", UnderscoreRun, "under");
    }

    #[test]
    fn equals_run_counts_from_one_to_four() {
        assert_run_lengths('=', EqualsRun);
    }

    #[test]
    fn equals_inside_text_splits_into_separate_tokens() {
        assert_glued_text("text=equals", EqualsRun, "equals");
    }

    #[test]
    fn backtick_run_counts_from_one_to_four() {
        assert_run_lengths('`', BacktickRun);
    }

    #[test]
    fn backtick_inside_text_splits_into_separate_tokens() {
        assert_glued_text("text`code", BacktickRun, "code");
    }

    #[test]
    fn tilde_run_counts_from_one_to_four() {
        assert_run_lengths('~', TildeRun);
    }

    #[test]
    fn tilde_inside_text_splits_into_separate_tokens() {
        assert_glued_text("text~tilde", TildeRun, "tilde");
    }

    #[test]
    fn gt_run_counts_from_one_to_four() {
        assert_run_lengths('>', GtRun);
    }

    #[test]
    fn gt_inside_text_splits_into_separate_tokens() {
        assert_glued_text("text>quote", GtRun, "quote");
    }

    #[test]
    fn adjacent_special_character_runs_remain_separate() {
        assert_eq!(
            tokens("---***___===```~~~>>>", FrontmatterAllowed::No),
            vec![
                Token::new(DashRun, 3),
                Token::new(StarRun, 3),
                Token::new(UnderscoreRun, 3),
                Token::new(EqualsRun, 3),
                Token::new(BacktickRun, 3),
                Token::new(TildeRun, 3),
                Token::new(GtRun, 3),
            ]
        );
    }

    #[test]
    fn whitespace_is_consumed_as_a_single_run() {
        let toks = tokens("a    b", FrontmatterAllowed::No);
        assert_eq!(
            toks,
            vec![
                Token::new(Text, 1),
                Token::new(Whitespace, 4),
                Token::new(Text, 1),
            ]
        );
    }

    #[test]
    fn line_endings_produce_newline_tokens() {
        assert_eq!(
            tokens("a\nb", FrontmatterAllowed::No)[1],
            Token::new(Newline, 1)
        );
        assert_eq!(
            tokens("a\rb", FrontmatterAllowed::No)[1],
            Token::new(Newline, 1)
        );
        assert_eq!(
            tokens("a\r\nb", FrontmatterAllowed::No)[1],
            Token::new(Newline, 2)
        );
    }

    #[test]
    fn unicode_text_length_is_measured_in_bytes() {
        let input = "café"; // 'é' is 2 bytes in UTF-8, so this is 5 bytes / 4 chars.
        assert_eq!(input.len(), 5);
        let toks = tokens(input, FrontmatterAllowed::No);
        assert_eq!(toks, vec![Token::new(Text, input.len() as u32)]);
    }

    #[test]
    fn valid_frontmatter_is_recognized() {
        let opening = "---\ntitle: x\n---\n";
        let input = format!("{opening}body");
        let toks = tokens(&input, FrontmatterAllowed::Yes);
        assert_eq!(
            toks,
            vec![
                Token::new(Frontmatter { terminated: true }, opening.len() as u32),
                Token::new(Text, "body".len() as u32),
            ]
        );
        assert!(!toks.iter().any(|token| token.kind == DashRun));
    }

    #[test]
    fn frontmatter_ignores_a_line_that_only_looks_like_a_closing_fence() {
        // "---invalid" starts with three dashes but isn't a real fence line
        // (nothing but whitespace may follow before the line ending), so it
        // doesn't close the frontmatter. With no real closer, the rest of
        // the document is swallowed as an unterminated frontmatter block.
        let input = "---\n---invalid\nreal body\n";
        let toks = tokens(input, FrontmatterAllowed::Yes);
        assert_eq!(
            toks,
            vec![Token::new(
                Frontmatter { terminated: false },
                input.len() as u32
            )]
        );
    }

    #[test]
    fn frontmatter_only_recognized_at_document_start() {
        let input = "a\n---\ntitle: x\n---\n";
        let toks = tokens(input, FrontmatterAllowed::Yes);
        assert!(!toks.iter().any(|t| matches!(t.kind, Frontmatter { .. })));
        assert_eq!(toks[2], Token::new(DashRun, 3));
    }

    #[test]
    fn leading_dashes_are_a_run_when_frontmatter_is_not_allowed() {
        let toks = tokens("---\nbody", FrontmatterAllowed::No);
        assert_eq!(
            toks,
            vec![
                Token::new(DashRun, 3),
                Token::new(Newline, 1),
                Token::new(Text, 4),
            ]
        );
    }

    #[test]
    fn unterminated_frontmatter_consumes_rest_of_input_and_is_marked_unterminated() {
        let input = "---\ntitle: x\n";
        let toks = tokens(input, FrontmatterAllowed::Yes);
        assert_eq!(
            toks,
            vec![Token::new(
                Frontmatter { terminated: false },
                input.len() as u32
            )]
        );
    }

    #[test]
    fn frontmatter_after_utf8_bom_is_recognized() {
        let opening = "\u{feff}---\ntitle: x\n---\n";
        let input = format!("{opening}body");
        let toks = tokens(&input, FrontmatterAllowed::Yes);
        assert_eq!(
            toks[0],
            Token::new(Frontmatter { terminated: true }, opening.len() as u32)
        );
    }

    #[test]
    fn token_lengths_sum_to_input_length() {
        let inputs = [
            "",
            "hello world",
            "# heading\n",
            "text#hash and  spaces\r\n",
            "café ☕ com emoji 🦀",
            "---\ntitle: x\n---\nbody",
            "\u{feff}---\ntitle: x\n---\nbody",
            "---\n---invalid\nreal body\n",
            "---***___===```~~~>>>",
            "text-dash*star_under=equals`code~tilde>quote",
            "a\n----\nb",
        ];

        for input in inputs {
            let total: u32 = tokens(input, FrontmatterAllowed::Yes)
                .iter()
                .map(|t| t.len)
                .sum();
            assert_eq!(total as usize, input.len(), "input: {input:?}");
        }
    }
}
