## ADDED Requirements

### Requirement: Cargo workspace covers both crates
The root `Cargo.toml` SHALL declare a `[workspace]` table with
`members = ["crates/context-lexer", "crates/context-parser"]`, so that `cargo build`, `cargo test`,
`cargo clippy`, and `cargo fmt` invoked from the repository root operate on both crates without
needing per-crate `--manifest-path` flags.

#### Scenario: Root-level test run covers both crates
- **WHEN** `cargo test --workspace` is run from the repository root
- **THEN** test binaries are built and run for both `context-lexer` and `context-parser`

#### Scenario: Placeholder binary remains buildable
- **WHEN** `cargo build --workspace` is run from the repository root
- **THEN** the existing `context` binary (`src/main.rs`) still builds, unaffected by the workspace
  change

### Requirement: `/check` command for the cargo dev loop
A Claude Code command SHALL provide a single entry point that runs the standard cargo dev-loop
(format check, lint, test) across the workspace, and SHALL accept an optional crate name to scope
the run to one crate instead of the whole workspace.

#### Scenario: Whole-workspace check
- **WHEN** the `/check` command is invoked with no arguments
- **THEN** it runs `cargo fmt --check`, `cargo clippy`, and `cargo test` across the whole workspace
  and reports the combined result

#### Scenario: Single-crate check
- **WHEN** the `/check` command is invoked with a crate name argument (e.g. `context-lexer`)
- **THEN** it scopes `cargo fmt --check`, `cargo clippy`, and `cargo test` to that crate only

### Requirement: Changelog reminder on crate source changes
When a Claude Code turn modifies files under `crates/<name>/src/**` for some crate `<name>`, the
workflow SHALL check whether `crates/<name>/CHANGELOG.md` was also modified in that same turn, and
SHALL surface a non-blocking reminder if it was not, referencing the existing Keep-a-Changelog
`## [Unreleased]` format already used in both crates' changelogs.

#### Scenario: Source changed, changelog not updated
- **WHEN** a turn edits a file under `crates/context-parser/src/**` and does not touch
  `crates/context-parser/CHANGELOG.md`
- **THEN** a reminder is shown at the end of the turn prompting a Keep-a-Changelog entry, and the
  turn is NOT blocked

#### Scenario: Source and changelog both updated
- **WHEN** a turn edits a file under `crates/context-parser/src/**` and also edits
  `crates/context-parser/CHANGELOG.md`
- **THEN** no reminder is shown

#### Scenario: Non-source changes are ignored
- **WHEN** a turn only edits files outside every `crates/<name>/src/**` (e.g. `README.md` or
  `openspec/**`)
- **THEN** no changelog reminder is triggered

### Requirement: Format-on-edit for crate source files
Editing or writing a `.rs` file under `crates/**` SHALL automatically trigger `cargo fmt` scoped to
the owning crate, so formatting stays consistent without a manual step.

#### Scenario: Edit triggers formatting
- **WHEN** the Edit or Write tool modifies a file matching `crates/*/src/**/*.rs`
- **THEN** `cargo fmt -p <owning-crate>` runs automatically afterward

### Requirement: Generated entity table is protected from hand-edits
Direct edits to `crates/context-parser/src/inline/entities.rs` via the Edit or Write tool SHALL be
blocked, with a message directing the change to be made via
`crates/context-parser/tools/generate_entities.py` instead.

#### Scenario: Attempted hand-edit is blocked
- **WHEN** the Edit or Write tool targets `crates/context-parser/src/inline/entities.rs`
- **THEN** the tool call is blocked and a message references `tools/generate_entities.py` as the
  correct way to regenerate the file

#### Scenario: Regeneration via the generator script is unaffected
- **WHEN** `crates/context-parser/src/inline/entities.rs` is overwritten as a result of running
  `tools/generate_entities.py` (not via the Edit/Write tool directly)
- **THEN** the change is not blocked

### Requirement: Project context documented in CLAUDE.md
A root `CLAUDE.md` SHALL document the workspace layout, the lexer/parser responsibility split, the
edition-2024 toolchain, the Keep-a-Changelog/SemVer convention used by both crates, and the
generated-entities workflow, so this context does not need to be re-derived each session.

#### Scenario: CLAUDE.md covers the required topics
- **WHEN** `CLAUDE.md` is read
- **THEN** it describes the workspace layout, the `context-lexer`/`context-parser` split, the
  changelog convention, and the `generate_entities.py` code-generation step
