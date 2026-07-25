# context

A Cargo workspace implementing a structural context engine for Notes Markdown and exposing it to
AI agents over MCP.

## Workspace layout

- `Cargo.toml` (root) — defines both a `[workspace]` (members: the three crates below) and a
  `[package]` for the `context` MCP server binary (`src/main.rs`). Root-level `cargo build`,
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
- `crates/context-engine` — depends on `context-parser` and derives span-backed section trees,
  interprets YAML frontmatter, indexes Markdown vaults, and provides outlines, exact section
  retrieval, deterministic fuzzy heading search, hash-guarded section editing
  (`VaultIndex::edit_section`: body-only replacement verified against fresh disk bytes, atomic
  write, single-document reindex), and a vault-wide wikilink graph (`VaultIndex::backlinks`).
  Wikilink resolution (`[[Note]]`, `[[Note#Heading]]`, `[[#Heading]]`) is an **engine-level
  convention** — the parser hands over the target as one opaque span; splitting on `#`, stem
  matching a file part vault-wide, and matching a heading part by heading text are all decided
  and implemented in `context-engine`, not `context-parser`. Unresolved links (no match, or
  ambiguous) are non-fatal diagnostics, never a build failure. The link index is a whole-vault
  derived structure, rebuilt in full after every `edit_section`/`reindex_file` — a change to one
  file's headings can flip whether an unrelated file's link resolves, so partial invalidation
  would be incorrect. There is no write-side link fix-up yet (renaming a heading does not update
  inbound wikilinks).
  `VaultIndex::reindex_vault` is the only operation that re-reads disk on its own: it re-runs
  `VaultIndex::build` against the same root and diffs the fresh index against the one being
  replaced (files added/removed; per changed file, sections added/removed/modified by
  `content_hash`). Reporting uses opposite-direction "root cause only" filtering — a modified
  heading path is suppressed if a changed descendant exists (hash cascades upward through
  nesting), an added/removed heading path is suppressed if its parent is also added/removed
  (only the top of a new or deleted subtree is reported). A rename (file or heading) is always
  reported as a plain delete + add, never detected as a rename, matching the identity model used
  everywhere else in the engine. This is a known-imprecise heuristic, not a guarantee — see
  `crates/context-engine/src/diff.rs`.

The formerly open purpose of `src/main.rs` is resolved: it is the MCP server / CLI front-end for
`context-engine`. Start it on stdio with:

```
cargo run -- serve <vault-dir>
```

The server indexes the vault at startup and advertises `outline`, `get_section`, `search`,
`edit_section`, `backlinks`, and `reindex_vault` tools until the stdio transport closes. Reads
share the index behind a read-write lock; `edit_section` and `reindex_vault` take the write side.
`edit_section` replaces one section's body guarded by the `content_hash` carried in every
`get_section`/`search` provenance; `reindex_vault` re-walks the vault root and returns a diff —
the only way an agent learns about changes made outside the server (another editor, a git
checkout) without restarting it.

Edition 2024 throughout.

## Dev loop

Run from the repo root — the workspace covers both crates:

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --check
```

Or use the `/check` command (optionally scoped to one crate: `/check context-lexer`).

## Changelog convention

All three crates keep a `CHANGELOG.md` following
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
[SemVer 2.0.0](https://semver.org/spec/v2.0.0.html). When you change a crate's source, add or update
its `## [Unreleased]` section:

- A "Provisional version impact" line (e.g. `**MINOR** (0.2.0)`) stating the SemVer bump the
  unreleased changes imply.
- `### Added` / `### Changed` / `### Fixed` subsections listing the actual changes.

A `Stop` hook reminds you if a crate's `src/**` changed in a turn but its `CHANGELOG.md` didn't. It
diffs against `git status`, so it needs a commit baseline to compare against — right after the
repo's first commit it works as intended, but before any commit exists every file looks "changed"
(untracked), so the reminder can't distinguish a real gap from repo bootstrapping.
