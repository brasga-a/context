## Why

This repo (`context`, a Notes-Markdown lexer/parser workspace) has no project-specific Claude Code
setup yet — only OpenSpec's own `.claude/commands/opsx/*` and `.claude/skills/openspec-*`
scaffolding is present. There is no `CLAUDE.md`, no shortcuts for the per-crate cargo dev loop, no
enforcement of the Keep-a-Changelog/SemVer discipline the two crates already follow, and no hooks
to keep formatting/linting and generated files consistent. Separately, `crates/context-lexer` and
`crates/context-parser` are not wired into a Cargo workspace, so `cargo build`/`test`/`clippy` run
from the repo root silently skip both real crates. Fixing the workspace is a prerequisite for any
dev-loop command or hook that assumes `cargo <cmd> -p <crate>` works from the root.

## What Changes

- **BREAKING**: Add `[workspace]` to the root `Cargo.toml` with `members = ["crates/context-lexer",
  "crates/context-parser"]`, so root-level `cargo build`/`test`/`clippy`/`fmt` cover both crates.
  Decide what happens to the placeholder `context` binary (`src/main.rs`) — keep it as a workspace
  member or remove it if it's not serving a purpose yet.
- Add a root `CLAUDE.md` documenting: workspace layout, the lexer/parser split (context-free
  streaming lexer vs. source-backed parser), edition 2024, the Keep-a-Changelog + SemVer convention
  visible in both crates' `CHANGELOG.md`, and the code-generation step in
  `crates/context-parser/tools/generate_entities.py`.
- Add Claude Code custom commands/skills for the cargo dev loop (e.g. build/test/clippy/fmt across
  the workspace or scoped to one crate).
- Add a command/skill that enforces changelog discipline: when source changes touch a crate,
  prompt for/verify an `[Unreleased]` entry in that crate's `CHANGELOG.md` following the existing
  Keep-a-Changelog format and SemVer-impact annotation style.
- Add hooks that run `cargo fmt`/`cargo clippy` around edits to crate source, and that guard the
  generated entity table (`crates/context-parser/src/inline/entities.rs`) from being hand-edited
  instead of regenerated via `tools/generate_entities.py`.

## Capabilities

### New Capabilities
- `claude-dev-workflow`: Project-specific Claude Code workflow for this repo — cargo dev-loop
  commands, changelog-discipline enforcement, and format/lint/generated-file hooks, built on a
  corrected Cargo workspace.

### Modified Capabilities
(none — no existing specs in `openspec/specs/`)

## Impact

- `Cargo.toml` (root): gains `[workspace]` table; `src/main.rs` disposition decided.
- New: root `CLAUDE.md`.
- New: `.claude/commands/**` and/or `.claude/skills/**` entries for the dev-loop and changelog
  commands (additive, alongside existing `opsx/*` scaffolding).
- New/modified: `.claude/settings.json` (or equivalent) hook configuration for fmt/clippy and the
  generated-entities guard.
- No changes to `context-lexer` or `context-parser` crate source/behavior.
