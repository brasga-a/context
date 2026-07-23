# context-lexer

`context-lexer` is the low-level streaming lexer for Notes Markdown. It emits context-free token runs with byte lengths and leaves block and inline semantics to the parser.

Frontmatter detection at the true document start is its only context-sensitive rule.
