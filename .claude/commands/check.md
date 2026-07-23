---
name: "Check"
description: Run the cargo dev-loop (fmt check, clippy, test) across the workspace or one crate
allowed-tools: Bash(cargo:*)
category: Development
tags: [cargo, check, test, lint]
---

Run the project's standard cargo dev-loop: format check, lint, and tests.

**Input**: Optional crate name after `/check` (`context-lexer` or `context-parser`). If omitted,
run across the whole workspace.

**Steps**

1. If no crate name argument was given, run:
   ```bash
   cargo fmt --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```

2. If a crate name argument was given (e.g. `/check context-lexer`), scope every step to that
   crate instead:
   ```bash
   cargo fmt --check -p <crate>
   cargo clippy -p <crate> --all-targets -- -D warnings
   cargo test -p <crate>
   ```

3. Run all three steps even if an earlier one fails, and report a combined pass/fail summary at
   the end (which step(s) failed, and the relevant error output) rather than stopping at the first
   failure.

The workspace's baseline `cargo clippy` is currently warning-free, so `-D warnings` is expected to
pass cleanly on unrelated code — a failure means the change under review introduced a new lint.
