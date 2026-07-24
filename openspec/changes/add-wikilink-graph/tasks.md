## 1. Link extraction (context-engine)

- [x] 1.1 Implement a recursive inline-and-block walker collecting every `WikiLink` (span, target
      span, label span) from a parsed `Document`: paragraphs, headings, list items, table cells,
      blockquotes, footnote definitions; unit tests covering each container type plus nesting
      (wikilink inside a list item inside a blockquote)
- [x] 1.2 Implement `section_at(sections, offset) -> Option<&Section>`: deepest section whose span
      contains a byte offset, mirroring the recursive-containment shape of `find_section` /
      `visit_sections`; unit tests including preamble coverage and nested children
- [x] 1.3 Implement target-text splitting: file part / heading part on first `#`, empty file part
      → current file, no `#` → whole-file; unit tests per spec scenarios

## 2. Resolution (context-engine)

- [x] 2.1 Implement file resolution: no-`/` file part → stem match vault-wide; `/`-containing
      file part → full vault-relative path match (with/without trailing `.md`); zero or multiple
      matches → unresolved; unit tests (unique stem, ambiguous stem across directories, full-path
      match, no match)
- [x] 2.2 Implement heading resolution within a resolved file: match by section heading text;
      zero or multiple matches → unresolved with candidate heading paths; unit tests (unique
      match, duplicate heading text ambiguity, no match)
- [x] 2.3 Wire 1.1–1.3 and 2.1–2.2 into a whole-vault link-resolution pass operating over the
      in-memory `documents` map (re-parses each source via `context_parser::parse` per design
      Decision 5); unit tests: resolved whole-file link, resolved file+heading link, resolved
      self-link, each unresolved variant

## 3. Backlink index and reindex correctness (context-engine)

- [x] 3.1 Add `Backlink { from: Provenance, raw_target: String, target_heading_path: Option<String> }`
      and `VaultIndex.backlinks: BTreeMap<String, Vec<Backlink>>`; populate from the resolution
      pass keyed by resolved target file
- [x] 3.2 Route unresolved links into `VaultIndex.diagnostics` as `VaultDiagnostic` entries naming
      source file, linking section, and unresolved target text
- [x] 3.3 Rebuild the full link/backlink index (not just the touched file) at the end of
      `VaultIndex::build` and after every successful `VaultIndex::edit_section` /
      `VaultIndex::reindex_file`; regression test for the cross-file scenario in design Decision 4
      (editing file A's headings changes file B's link resolution without touching B)
- [x] 3.4 Add `VaultIndex::backlinks(file, heading_path: Option<&str>) -> Vec<Backlink>` (or
      equivalent), filtering by target heading path when supplied; integration tests against a
      fixture vault

## 4. MCP tool (context binary)

- [x] 4.1 Add wikilink fixtures to `crates/context-engine/tests/fixtures/vault`: a resolved
      whole-file link, a resolved file+heading link, a self-link, an unresolved-file link, an
      unresolved-heading link, and an ambiguous-heading case
- [x] 4.2 Implement the `backlinks` tool (file, optional heading_path) on the read lock, alongside
      `outline` / `get_section` / `search`; response includes full provenance, raw target text,
      and resolved target heading path per backlink
- [x] 4.3 End-to-end verification over stdio against the expanded fixture vault: backlinks for a
      whole file, backlinks narrowed to one heading path, and a diagnostic present for the
      unresolved-link fixtures

## 5. Wrap-up

- [x] 5.1 Update `CLAUDE.md`: fifth tool `backlinks`, note the wikilink resolution conventions
      (file part / heading part / self-link) as engine-introduced, not parser-level
- [x] 5.2 Update changelogs per convention: `context-engine` (**MINOR** — new read API), root
      binary changelog
- [x] 5.3 Run `/check` across the workspace; leave formatted, lint-clean, all tests passing
