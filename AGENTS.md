# context

A Cargo workspace implementing a Notes-Markdown lexer and parser.

## Workspace layout

- `Cargo.toml` (root) — defines both a `[workspace]` (members: the two crates below) and a
  `[package]` for a placeholder `context` binary (`src/main.rs`). Root-level `cargo build`,
  `cargo test`, `cargo clippy`, and `cargo fmt` cover everything in the workspace.
- `crates/context-lexer` — the low-level streaming lexer. Emits context-free token runs with byte
  lengths; the only context-sensitive rule it applies is frontmatter detection at the true document
  start. Block/inline semantics are the parser's job, not the lexer's.
- `crates/context-parser` — depends on `context-lexer` and turns its token stream into a
  source-backed Markdown document tree (AST with absolute source spans). Owns block parsing,
  inline parsing, diagnostics, and link/footnote definitions. Supports CommonMark and GFM plus
  Notes extensions (wikilinks, highlight, math, frontmatter, footcontext).
  - `src/inline/entities.rs` is **generated** by `tools/generate_entities.py` (produces the
    WHATWG semicolon-terminated HTML entity table). Never hand-edit this file — rerun the script
    instead.

Edition 2024 throughout.

## Dev loop

Run from the repo root — the workspace covers both crates:

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --check
```

Or use the `/check` Codex command (optionally scoped to one crate: `/check context-lexer`).

## Changelog convention

Both crates keep a `CHANGELOG.md` following [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html). When you change a crate's source, add or
update its `## [Unreleased]` section:

- A "Provisional version impact" line (e.g. `**MINOR** (0.2.0)`) stating the SemVer bump the
  unreleased changes imply.
- `### Added` / `### Changed` / `### Fixed` subsections listing the actual changes.

A `Stop` hook reminds you if a crate's `src/**` changed in a turn but its `CHANGELOG.md` didn't. It
diffs against `git status`, so it needs a commit baseline to compare against — right after the
repo's first commit it works as intended, but before any commit exists every file looks "changed"
(untracked), so the reminder can't distinguish a real gap from repo bootstrapping.
