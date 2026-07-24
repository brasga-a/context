## Context

`context-parser` emits `Inline::WikiLink { span, target, label }` for `[[Note]]` and
`[[Note|label]]` (`crates/context-parser/src/inline/mod.rs`); `target` is an opaque span — the
parser does not special-case `#`, and no vault fixture or doc in this repo uses wikilinks yet, so
every resolution rule below is a **new convention this change introduces**, not one discovered in
existing usage. `EngineDocument` currently discards the parsed `Document` after deriving
`sections: Vec<Section>` — `Section` keeps span, heading text, and hash, but not the original
block/inline tree, so nothing today can find a `WikiLink` node again without re-parsing. This
change was scoped in the 2026-07-24 exploration session: read-only graph now (index + backlinks
tool), write-side fix-up (rewriting links after a future rename) explicitly deferred until this
foundation has seen real use.

## Goals / Non-Goals

**Goals:**
- Engine-level resolution of `[[Note]]`, `[[Note#Heading]]`, and `[[#Heading]]` to a file and
  (optionally) a section within it.
- A vault-wide backlink index: for any file (optionally narrowed to a heading path), every
  resolved link pointing at it, with full provenance.
- Unresolved links (no match, or ambiguous match) recorded as non-fatal diagnostics, never a
  build failure.
- Correctness across edits: after any successful `edit_section`, the backlink index reflects
  reality vault-wide, not just for the edited file.
- `backlinks(file, heading_path?)` MCP tool.

**Non-Goals:**
- No write-side link fix-up (rewriting `[[Old#Heading]]` after a rename) — no `rename_section`
  exists yet, and cross-file atomic writes are a materially bigger primitive than anything built
  so far (every write to date is single-file). Follow-up change once `rename_section` exists.
- No outbound `links_from` tool — an agent can already see raw wikilink text in `get_section`'s
  returned content; only the reverse direction (backlinks) is missing.
- No block-reference (`^id`) or other Obsidian-specific link extensions beyond `#Heading`.
- No fuzzy/typo-tolerant target matching — resolution is exact-stem / exact-heading-text, with
  ambiguity surfaced as candidates rather than guessed at (consistent with `get_section`'s exact
  lookup, not `search`'s fuzzy one).

## Decisions

### 1. Target syntax: split on the first `#`, engine-side only
The parser keeps handing over `target` as one opaque `Span` — no parser change. The engine splits
the sliced target text on its first `#`: everything before is the file part, everything after is
the heading part (absent `#` → whole-file link, heading part `None`). Empty file part
(`[[#Heading]]`) means "resolve within the current file".
- **Alternative considered**: teach the parser about `#` — rejected; `context-parser`'s contract
  is "source-backed, no interpretation" (the same reasoning that kept sections out of the
  parser). A link-target convention is exactly the kind of interpretation that belongs one layer
  up.

### 2. File resolution: stem match, vault-wide, ambiguity is unresolved not an error
If the file part contains no `/`, it is matched against the filename stem (path segment minus
`.md`) of every indexed file, vault-wide — not scoped to the linking file's directory. If the file
part contains `/`, it is matched as a full vault-relative path (with or without a trailing `.md`).
Exactly one match resolves; zero or more than one is "unresolved" (a diagnostic, not a build
failure — vaults keep indexing around bad links, same posture as unreadable files).
- **Alternative considered**: always require a full vault-relative path — rejected as needless
  ceremony for the common case (a small vault, mostly-unique note names); the ambiguous case still
  degrades safely to "unresolved" rather than guessing.
- Matching is exact (case-sensitive), not fuzzy — deliberately narrower than `search`'s token
  matching, matching `get_section`'s exact-lookup posture instead. A future change can loosen this
  if real vaults show a need; nothing here forecloses it.

### 3. Heading resolution: match on heading text, not full heading-path
The heading part matches against the resolved file's section **heading text** (`Section.heading`),
not its disambiguated `heading_path`. Two sections with the same heading text in that file (e.g.
both named `Gun`, disambiguated internally as `Gun` and `Gun[2]`) make `[[File#Gun]]` ambiguous —
unresolved, with both heading paths listed as candidates (reusing the existing
`RetrievalError`-style suggestions convention).
- **Alternative considered**: require the caller to write the full breadcrumb
  (`[[File#Skills > Gun]]`) — rejected; that breaks the plain-Obsidian-convention expectation for
  no benefit, since ambiguity is already handled safely by degrading to "unresolved".

### 4. The link index is a whole-vault derived structure, rebuilt in full on any single-file change
This is the load-bearing decision. A naive "reindex only the touched file" is wrong: editing file
A can add or remove headings in A (an `edit_section` body may legally introduce new child
headings — see the section-editing change), which flips whether some *other* file B's
`[[A#Heading]]` link resolves, even though B itself never changed. Because backlinks are a
cross-file derived index, partial invalidation is a correctness hazard, not an optimization. The
link index is therefore recomputed from the full in-memory `documents` map every time
`VaultIndex::build` runs and after every successful `edit_section` / `reindex_file` — a pure,
disk-free pass over already-parsed sources (parsing a vault is millisecond-scale per the
foundation change's measurements; re-deriving links is cheaper than the reparse already done).
- **Alternative considered**: track per-file forward links and only invalidate backlink entries
  whose source file changed — rejected; it does not address the A-changed/B's-link-flips case
  above, and a vault-scale full rebuild is cheap enough that the extra complexity buys nothing.

### 5. Link extraction re-parses; `EngineDocument` is not widened
Finding `WikiLink` nodes requires walking the full block/inline tree (paragraphs, headings, list
items, table cells, blockquotes, footnote definitions) — a traversal that does not exist yet
(`Section` retains only span/heading text/hash, not the original nodes). The link-index build
calls `context_parser::parse` again on each document's source specifically for this walk, rather
than widening `EngineDocument` to retain the parsed `Document`.
- **Alternative considered**: store the parsed `Document` on `EngineDocument` — rejected for v1;
  `Document` does derive `Clone, PartialEq, Eq` so it's mechanically possible, but it widens a
  type every existing consumer depends on for a need only the link indexer has, and doubles the
  cost of every `EngineDocument` clone. Re-parsing is one extra millisecond-scale pass; revisit
  only if profiling ever shows it matters.
- Each found `WikiLink`'s owning section is the deepest section whose span contains the link's
  span start — a new `section_at(sections, offset)` helper, the same recursive-containment shape
  already used by `find_section`/`visit_sections` in `vault.rs`. The synthetic preamble section
  covers pre-heading content, so links there resolve to `heading_path: "Preamble"` with no special
  casing needed.

### 6. Backlink storage and the tool's response shape
`VaultIndex` gains a `backlinks: BTreeMap<String, Vec<Backlink>>` keyed by resolved target file,
where `Backlink { from: Provenance, raw_target: String, target_heading_path: Option<String> }`.
`backlinks(file, heading_path?)` looks up the target file's vector and, when `heading_path` is
given, filters to entries whose `target_heading_path` matches. `Provenance` (already carrying
`content_hash` since the section-editing change) is reused verbatim — no new provenance type.

## Risks / Trade-offs

- **Re-parsing for link extraction doubles per-file parse cost on every build/edit.** Accepted:
  the foundation change already established vault parsing as millisecond-scale; this is one more
  pass of the same order, not a new complexity class.
- **Full link-index rebuild on every edit is O(vault size), not O(1).** Accepted for the same
  reason — and it is the only option that is straightforwardly correct (Decision 4).
- **Exact-match resolution will feel strict compared to `search`'s fuzzy matching** — a typo'd
  wikilink target simply becomes "unresolved" rather than suggesting a fix. This is deliberate
  (Decision 2/3) but worth flagging: it's a UX ceiling to revisit once real vaults exercise it.
- **No fix-up on rename** means renaming a heading by hand (outside `edit_section`, or via a
  future `rename_section`) silently breaks inbound links until the next read surfaces them as
  diagnostics. Acceptable for a read-only change; the proposal names this as the explicit
  follow-up.

## Migration

No data or API migration. Existing read tools (`outline`, `get_section`, `search`) and
`edit_section` are unchanged. New engine API and MCP tool are additive (MINOR).

## Open Questions

(none — scope was deliberately narrowed to read-only in exploration; deferred items are listed in
the proposal)
