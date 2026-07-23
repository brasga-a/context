## 1. Crate scaffolding

- [ ] 1.1 Create `crates/context-engine` (lib crate, edition 2024, MIT, repository field matching
      the other crates) depending on `context-parser`; add it to the root workspace `members`;
      seed `README.md` and `CHANGELOG.md` in the established Keep-a-Changelog format
- [ ] 1.2 Verify `cargo build --workspace` and `/check` pass with the empty crate wired in

## 2. Section tree (context-engine)

- [ ] 2.1 Implement section-tree derivation from `Document.children`: nesting by heading level,
      synthetic preamble section for pre-heading content, skipped-level handling; section span =
      heading start → end of last contained block
- [ ] 2.2 Implement heading-path identity (breadcrumbs from heading text) with deterministic
      `[n]` disambiguation for duplicate sibling headings
- [ ] 2.3 Implement blake3 content hashing over each section's span bytes
- [ ] 2.4 Unit tests against fixture documents: nesting shape, preamble, skipped levels, span
      byte-exactness (slice == original text), duplicate-path disambiguation, hash
      stability/sensitivity

## 3. Frontmatter metadata (context-engine)

- [ ] 3.1 Parse the `FrontmatterBlock` span as YAML into document metadata (choose maintained
      serde-YAML crate per design note); lenient failure — diagnostic + absent metadata, never a
      failed index
- [ ] 3.2 Unit tests: valid frontmatter, invalid YAML tolerated with diagnostic, absent
      frontmatter

## 4. Vault index and structural retrieval (context-engine)

- [ ] 4.1 Implement vault indexing: walk a directory tree, parse every `.md` file into its section
      tree keyed by vault-relative path; unreadable files skipped with diagnostics; non-`.md`
      files ignored
- [ ] 4.2 Implement outline generation (heading text, level, path, line range via the parser's
      line index — no body content)
- [ ] 4.3 Implement exact retrieval: (file, heading path) → byte-exact source slice + provenance;
      not-found errors carry the missing name and nearest-match suggestions
- [ ] 4.4 Implement fuzzy heading search: case-insensitive token normalization; ranking exact path
      > exact heading > token-subset/substring; deterministic tie-breaking (path depth, then file
      path); results carry full provenance
- [ ] 4.5 Unit tests against a fixture vault: indexing (nested dirs, non-md ignored), outline
      shape, exact retrieval + helpful errors, fuzzy cases from the spec (`gun skill`,
      `skill gun`, `Gun`), ranking determinism

## 5. MCP server (context binary)

- [ ] 5.1 Add `rmcp` (pinned) and `context-engine` to the root binary; implement
      `context serve <vault-dir>` CLI with fail-fast validation of the vault path
- [ ] 5.2 Implement the MCP stdio server advertising `outline`, `get_section`, and `search`
      tools mapped onto the engine, with retrieval errors surfaced as tool errors (message +
      suggestions)
- [ ] 5.3 End-to-end verification: drive the running server over stdio with a scripted MCP client
      session (initialize → outline → get_section `Skills > Gun` → search `gun skill`) against a
      fixture vault; confirm byte-exact section content and error behavior for a bad path

## 6. Wrap-up

- [ ] 6.1 Update `CLAUDE.md`: `context-engine` crate role, the binary's MCP server purpose and
      `serve` usage; note the resolved `src/main.rs` open question
- [ ] 6.2 Update `CHANGELOG.md` files (`context-engine` new; root binary if it keeps one) per the
      changelog convention
- [ ] 6.3 Run `/check` across the workspace; leave the repo formatted, lint-clean, all tests
      passing
