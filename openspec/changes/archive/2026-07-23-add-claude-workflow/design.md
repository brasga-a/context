## Context

The repo has no commits yet (`git log` shows an empty `master`), so there is no history/rollback
complexity to protect — this change effectively establishes the first real project conventions.
Two real crates exist (`context-lexer`, `context-parser`, a Notes-Markdown lexer/parser pair) plus
a placeholder `context` binary (`src/main.rs`, still "Hello, world!"). The root `Cargo.toml` has no
`[workspace]` table, so `cargo build/test/clippy/fmt` from the repo root today only ever touches
the placeholder binary. Both crates already follow Keep-a-Changelog + SemVer in their
`CHANGELOG.md`, and `context-parser` has a generated file
(`src/inline/entities.rs`, produced by `tools/generate_entities.py`) that must not be hand-edited.
The only existing `.claude/` content is OpenSpec's own `opsx` commands/skills — nothing
project-specific.

## Goals / Non-Goals

**Goals:**
- Make root-level `cargo build/test/clippy/fmt` cover both real crates, without deciding the fate
  of the placeholder binary.
- Give Claude (and the user) fast, explicit commands for the cargo dev loop.
- Nudge changelog updates when crate source changes, without blocking every edit.
- Auto-format on edit; keep clippy out of the hot path (run it via command/Stop hook instead).
- Block accidental hand-edits to the generated entity table.
- Document the project once, in `CLAUDE.md`, so this context doesn't need to be re-derived per
  session.

**Non-Goals:**
- No changes to crate source, behavior, or public APIs.
- No CI/GitHub Actions setup (not requested; can be a follow-up change).
- No decision to remove/repurpose `src/main.rs` — it becomes a workspace member as-is.
- No enforcement mechanism that can't be bypassed — hooks here are guardrails/reminders, not a
  substitute for review.

## Decisions

### 1. Workspace fix: add `[workspace]` alongside the existing `[package]`, don't touch `src/main.rs`
Cargo supports a "root package that is also a workspace" — adding a `[workspace]` table with
`members = ["crates/context-lexer", "crates/context-parser"]` to the existing root `Cargo.toml`
makes the root package an implicit workspace member alongside the two listed crates. No need to
list the root package explicitly, and no need to decide whether `src/main.rs` still serves a
purpose. This was chosen over converting the root to a virtual manifest (which would require
either deleting `src/main.rs` or moving it into its own crate directory) because it's the smallest
change that unblocks root-level cargo commands.
- **Alternative considered**: virtual manifest (`[workspace]` only, no `[package]`) — rejected for
  now since it forces a decision about `src/main.rs` that's out of scope for this change.

### 2. Cargo dev-loop as slash commands, not skills
Skills auto-trigger based on task-matching and are better suited for "guidance that should kick in
implicitly." The user wants explicit, predictable actions ("run the check"), which maps to
`.claude/commands/*.md` — plain, user-invoked, no ambiguity about when they fire. A single
`/check` command (root-level workspace check) covers the common case; it accepts an optional crate
name argument to scope to one crate (`/check context-lexer`) via `cargo <cmd> -p <crate>`.
- **Alternative considered**: one command per crate (`/check-lexer`, `/check-parser`) — rejected as
  needless duplication; an optional argument does the same job with one file.

### 3. Changelog nudge as a `Stop` hook, not `PostToolUse`
Checking after every single `Edit`/`Write` call would fire mid-edit, before a logically complete
change is even written, and would be noisy on multi-file edits. A `Stop` hook (fires once when
Claude finishes responding) can look at the whole turn's diff at once: for each
`crates/<name>/src/**` path touched, confirm `crates/<name>/CHANGELOG.md` was also touched; if not,
print a non-blocking reminder (visible to Claude/user) rather than failing the turn. This mirrors
the format already established in both crates (`## [Unreleased]` + SemVer-impact line + Added/
Changed/Fixed).
- **Alternative considered**: blocking `PreToolUse`/`PostToolUse` hook per edit — rejected as too
  disruptive for a discipline that's advisory, not a hard invariant.

### 4. Format on edit, lint on demand
`cargo fmt -p <crate>` is fast and side-effect-free, so it runs as a `PostToolUse` hook on
`Edit`/`Write` matching `crates/**/*.rs`, applied automatically. `cargo clippy` is slower and its
output is more useful in bulk, so it's left to the `/check` command and the `Stop` hook's summary
rather than running on every keystroke-equivalent edit.

### 5. Generated-file guard as a blocking `PreToolUse` hook
`crates/context-parser/src/inline/entities.rs` is machine-generated. A `PreToolUse` hook matched on
`Edit`/`Write` targeting that exact path exits non-zero with a message pointing at
`tools/generate_entities.py`, blocking the tool call outright — this is the one place a hard block
(vs. a nudge) is appropriate, since there's no legitimate reason to hand-edit that file.
- **Known limitation**: a path-matched hook only guards the `Edit`/`Write` tools; it doesn't stop
  the same file being changed via `Bash` (e.g. `sed`). This is called out in `CLAUDE.md` as a
  convention, not a hard guarantee.

## Risks / Trade-offs

- [Workspace change is technically **BREAKING** per Cargo semantics] → Mitigated: no commits/
  consumers exist yet, so there's nothing to break in practice.
- [Baseline `cargo clippy` may already have warnings across ~3100 lines of existing parser code,
  so `-D warnings` in `/check` could fail immediately] → Mitigation: during implementation, run
  `cargo clippy --workspace` once to see the baseline before deciding whether `/check` hard-fails
  on warnings or just reports them (see Open Questions).
- [Auto-fmt-on-edit could reformat more than the lines just touched if no `rustfmt.toml` pins
  style] → Mitigation: rely on rustfmt defaults (both crates already appear consistently formatted
  in edition-2024 style); revisit only if noisy diffs show up in practice.
- [`Stop` hook changelog reminder could misfire on non-source edits, e.g. README-only changes] →
  Mitigation: scope the check strictly to `crates/*/src/**` vs. `crates/*/CHANGELOG.md`, not the
  whole repo diff.

## Migration Plan

No rollback complexity: this is additive tooling plus one small `Cargo.toml` edit, on a repo with
no prior commits. Implementation order: (1) workspace fix, verify `cargo build`/`test` from root
picks up both crates, (2) `CLAUDE.md`, (3) `/check` command, (4) hooks (fmt, changelog reminder,
generated-file guard), verified by triggering each once.

## Open Questions

- ~~Should `/check`'s clippy step use `-D warnings` (hard fail) or just report~~ — **Resolved**:
  the baseline `cargo clippy --workspace --all-targets` run was clean (zero warnings), so `/check`
  uses `-D warnings`.
- Does the placeholder `context` binary (`src/main.rs`) stay indefinitely, or is there a near-term
  plan for it (e.g. becoming a CLI front-end for the parser)? **Still open** — left untouched per
  decision #1; `/check` does not special-case it.

## Implementation Notes

- Fixing the workspace surfaced a pre-existing, unrelated bug: `context-parser`'s `Cargo.toml`
  still depended on the crate's old name (`notes-lexer`), which the crate's own `CHANGELOG.md`
  already documented as renamed to `context-lexer`/`context_lexer`. This blocked `cargo build`
  entirely once the workspace was wired up, so the rename was finished (the `Cargo.toml` dependency
  and three `notes_lexer::` imports) as part of task 1.2 — no behavior change, just completing an
  already-declared rename.
- The `Stop`-hook changelog reminder diffs against `git status`, which needs a commit baseline to
  distinguish "changed this turn" from "never committed." This repo has no commits yet, so the
  reminder can't be meaningfully exercised end-to-end until after the first commit; its logic was
  verified instead against a throwaway repo with a commit baseline. Documented as a caveat in
  `CLAUDE.md`.
