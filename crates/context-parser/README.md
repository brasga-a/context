# context-parser

`context-parser` transforms the `context-lexer` token stream into a source-backed Markdown document tree. It owns block and inline parsing, diagnostics, definitions, and absolute source spans.

The parser supports CommonMark and GFM constructs together with Notes extensions such as wikilinks, highlight, math, frontmatter, and footcontext.
