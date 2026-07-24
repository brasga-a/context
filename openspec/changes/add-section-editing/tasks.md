## 1. Engine write module (context-engine)

- [x] 1.1 Record `heading_span: Option<Span>` on `Section` during tree derivation (None for
      preamble; covers ATX line or setext content + underline); expose `content_hash` in
      `Provenance` for `get_section` and `search`; unit tests for heading-span correctness (ATX,
      setext, preamble) and provenance hashes
- [x] 1.2 Implement section-escape validation: parse new body standalone behind a leading-newline
      guard (defeats frontmatter masquerade), reject any top-level heading with level ≤ target
      level, error names offending heading + level; accept deeper and container-nested headings;
      tests cover ATX, setext, frontmatter-lookalike, blockquote-nested, equal/lower/deeper
      levels
- [x] 1.3 Implement the span-tight splice: replace exactly [end of heading line, span.end) with
      the body trimmed of trailing whitespace-only lines; preserve all outside bytes (separators
      included) verbatim; empty body leaves heading line only; plus post-splice skeleton
      verification (heading path + level of every section outside the edited one unchanged, else
      restructure error); unit tests per spec scenarios (trailing whitespace, verbatim interior,
      empty body, setext-glue rejection, CRLF heading terminator)
- [x] 1.4 Implement the guarded edit: re-read file from disk, re-derive tree, resolve heading
      path (not-found errors reuse nearest-match suggestions), compare BLAKE3 hash
      (conflict error carries current hash), splice body span, atomic write via temp file +
      rename in the same directory — verify replace semantics of `std::fs::rename` on Windows
      per design Decision 5
- [x] 1.5 Add `VaultIndex::reindex_file(&mut self, file)` re-parsing one document; wire the edit
      flow to reindex on success and leave index untouched on failure
- [x] 1.6 Integration tests against a fixture vault: hash-match edit (byte-identical outside
      body), stale-hash conflict, stale-index-fresh-disk success, unknown file/path, failed edit
      leaves file + index untouched, successful edit immediately retrievable

## 2. MCP tool (context binary)

- [x] 2.1 Wrap the server's `VaultIndex` in `RwLock`; read tools take read guards; behavior of
      `outline` / `get_section` / `search` unchanged
- [x] 2.2 Implement `edit_section(file, heading_path, body, expected_hash)` holding the write
      guard across verify → write → reindex; success returns the fresh outline with per-section
      content hashes; conflict / escape / not-found surfaced as tool errors with messages and
      suggestions
- [x] 2.3 End-to-end verification over stdio against a fixture vault: get_section → edit_section
      (success, confirm on-disk bytes and fresh hashes) → edit_section with stale hash
      (conflict) → escape-rejection case → get_section reflects the edit

## 3. Wrap-up

- [x] 3.1 Update `CLAUDE.md`: fourth tool `edit_section`, drop "read-only" phrasing for the
      server description
- [x] 3.2 Update changelogs per convention: `context-engine` (**MINOR** — new write API), root
      binary changelog if present
- [x] 3.3 Run `/check` across the workspace; leave formatted, lint-clean, all tests passing
