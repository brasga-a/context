# Contributing to context

Thanks for helping improve `context`. Contributions to the Markdown lexer, parser, context engine,
MCP server, tests, and documentation are welcome.

This guide describes the repository's current development conventions. Keep changes focused,
source-backed, and covered by tests appropriate to their risk.

## Prerequisites

You need:

- a Rust toolchain with Edition 2024 support;
- Cargo; and
- Python 3 only when regenerating the HTML entity table.

Clone the repository and verify the workspace builds:

```console
cargo build --workspace
```

## Workspace map

- The root `context` package provides the CLI and stdio MCP server in `src/`.
- `crates/context-engine` builds section trees, indexes Markdown vaults, and implements structural
  retrieval and section editing.
- `crates/context-lexer` is the low-level streaming lexer. It emits context-free token runs with
  byte lengths; document-start frontmatter detection is its only context-sensitive rule.
- `crates/context-parser` turns lexer tokens into a source-backed Markdown document tree. It owns
  block and inline semantics, diagnostics, definitions, and absolute source spans.

Put behavior in the narrowest appropriate layer. In particular, do not move block or inline
semantics into the lexer.

## Development workflow

1. Identify the smallest package that owns the behavior.
2. Add or update a test that demonstrates the expected result.
3. Implement the change while preserving existing source-span and byte-exactness guarantees.
4. Update the affected package's changelog when package source changes.
5. Run the complete development checks before submitting.

For parser and engine work, pay particular attention to:

- byte offsets remaining on UTF-8 character boundaries;
- spans slicing back to the exact original source;
- deterministic ordering and stable identities;
- diagnostics remaining non-fatal where documented; and
- vault-relative paths never escaping the configured vault root.

## Tests and checks

Run the full development loop from the repository root:

```console
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run all three commands even if an earlier command fails. Every failure should be resolved before a
pull request is ready.

During development, you can scope checks to one package:

```console
cargo fmt --check -p context-parser
cargo clippy -p context-parser --all-targets -- -D warnings
cargo test -p context-parser
```

Replace `context-parser` with `context`, `context-engine`, or `context-lexer` as needed. Run the
workspace-wide commands before submitting because changes can affect downstream packages.

Tests live beside modules for focused behavior and under `tests/` directories for integration
coverage. MCP protocol changes should include an end-to-end stdio case in `tests/mcp_stdio.rs`.
Reuse or extend existing fixtures when exact source bytes and spans matter.

## Changelogs and version impact

The root binary and each crate maintain a `CHANGELOG.md` following
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

When changing a package's `src/**`, update its `## [Unreleased]` section with:

- a `Provisional version impact` line identifying the expected SemVer bump; and
- an entry under `### Added`, `### Changed`, or `### Fixed`.

Update only the changelogs for packages whose public behavior or source changed. Documentation-only
changes normally do not require a version-impact update.

## Generated files

Do not edit `crates/context-parser/src/inline/entities.rs` manually. It is generated from the
WHATWG semicolon-terminated HTML entity data.

Regenerate it with:

```console
python crates/context-parser/tools/generate_entities.py <entities.json> crates/context-parser/src/inline/entities.rs
```

Review the generated diff and run the parser tests afterward.

## Documentation

Keep commands, MCP tool names, request fields, response examples, and capability claims aligned
with the executable behavior. The detailed user reference is
[`docs/mcp-cli-usage.md`](docs/mcp-cli-usage.md).

Use standard relative Markdown links so documentation works in repository viewers as well as local
editors.

## Pull requests

A good pull request:

- addresses one coherent problem;
- explains the motivation and user-visible behavior;
- includes tests for new behavior and regressions;
- updates relevant documentation and changelogs;
- avoids unrelated formatting or refactoring; and
- passes formatting, Clippy, and the full workspace test suite.

Call out compatibility implications, deliberate trade-offs, or follow-up work in the description.
Small, reviewable commits are appreciated.

## License

By submitting a contribution, you agree that it may be distributed under the repository's
[MIT License](LICENSE).
