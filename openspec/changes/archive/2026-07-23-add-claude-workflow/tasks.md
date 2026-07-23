## 1. Cargo workspace

- [x] 1.1 Add `[workspace]` with `members = ["crates/context-lexer", "crates/context-parser"]` to
      the root `Cargo.toml`, leaving the existing `[package]`/`src/main.rs` untouched
- [x] 1.2 Run `cargo build --workspace` and `cargo test --workspace` from the repo root; confirm
      both crates and the placeholder binary all build/test
- [x] 1.3 Run `cargo clippy --workspace` once to capture the baseline warning count (informs the
      `-D warnings` open question in design.md before writing the `/check` command) — baseline is
      **zero warnings**, so `/check` will use `-D warnings`

## 2. CLAUDE.md

- [x] 2.1 Write root `CLAUDE.md`: workspace layout, `context-lexer`/`context-parser` responsibility
      split, edition 2024, build/test entry points
- [x] 2.2 Document the Keep-a-Changelog + SemVer-impact convention used in both crates'
      `CHANGELOG.md` files
- [x] 2.3 Document the generated-entities workflow
      (`crates/context-parser/tools/generate_entities.py` → `src/inline/entities.rs`) and that the
      generated file must not be hand-edited

## 3. `/check` command

- [x] 3.1 Add `.claude/commands/check.md` running `cargo fmt --check`, `cargo clippy`, and
      `cargo test` across the workspace by default
- [x] 3.2 Support an optional crate-name argument that scopes all three steps to
      `-p <crate>` instead of `--workspace`
- [x] 3.3 Decide clippy strictness (`-D warnings` vs. report-only) based on the 1.3 baseline, and
      wire it into the command — used `-D warnings` since the baseline is clean
- [x] 3.4 Verify: run `/check` with no args and with a crate name, confirm both paths work

## 4. Hooks

- [x] 4.1 Add a `PostToolUse` hook (Edit/Write, path glob `crates/*/src/**/*.rs`) that runs
      `cargo fmt -p <owning-crate>` automatically
- [x] 4.2 Add a `Stop` hook that diffs the turn's changed files, and for each `crates/<name>/src/**`
      path touched without a matching `crates/<name>/CHANGELOG.md` change, prints a non-blocking
      reminder in the existing Keep-a-Changelog format
- [x] 4.3 Add a `PreToolUse` hook (Edit/Write, exact path
      `crates/context-parser/src/inline/entities.rs`) that blocks the call and points to
      `tools/generate_entities.py`
- [x] 4.4 Wire all hooks into `.claude/settings.json`
- [x] 4.5 Verify each hook once: edit a crate `.rs` file (confirm auto-fmt fires), finish a turn
      without touching a changelog (confirm reminder fires), attempt to edit `entities.rs` directly
      (confirm it's blocked) — all three verified directly against synthetic hook input; the
      changelog reminder needed a `--untracked-files=all` fix (caught during verification) and was
      confirmed correct against a commit baseline in a throwaway repo, since this repo has no
      commits yet to diff against

## 5. Wrap-up

- [x] 5.1 Re-run `/check` at the workspace level to confirm the new tooling itself leaves the repo
      in a clean (formatted, passing) state — `cargo fmt --check`, `cargo clippy -D warnings`, and
      `cargo test --workspace` (135 tests) all pass
- [x] 5.2 Note the two design.md open questions (clippy strictness, `src/main.rs` fate) as
      resolved or still-open based on what was decided during implementation — clippy strictness
      resolved (`-D warnings`, baseline was clean); `src/main.rs` fate left open, unaffected by this
      change per design decision #1
