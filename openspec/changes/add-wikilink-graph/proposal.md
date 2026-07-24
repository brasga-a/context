## Why

`context-parser` already tokenizes `[[Note]]` and `[[Note|label]]` as `Inline::WikiLink { target, label, .. }`, but the engine never looks at it: it is invisible to every read tool today, surfacing only as plain text inside a heading via `inline_text`. An agent asking "what else references `Player > Skills > Gun`" — a natural question once an agent is editing a vault of cross-linked notes — has no way to answer it short of grepping every file's raw content itself, defeating the point of a structural engine. This change builds the read-only half of wikilink support: resolving link targets to sections and exposing a `backlinks` lookup. Write-side consequences (fixing links after a future `rename_section`) are explicitly deferred — the vault has zero wikilink usage today, so the resolution rules below are new conventions being introduced, not conventions being discovered, and getting them right needs real usage before an automated rewrite is safe.

## What Changes

- Wikilink **resolution rules** (new engine conventions, no prior art in this vault or the parser):
  - Target syntax `Note#Heading Text` (engine-level split on the first `#`; the parser continues
    to hand over the whole target as one opaque span). No `#` means a whole-file reference.
    Empty file part (`[[#Heading Text]]`) means "this same file".
  - File resolution: the file part matches by filename stem against every indexed file, vault-wide
    (not scoped to the linking file's directory). Zero matches or more than one match is
    "unresolved", not an error — the vault keeps indexing.
  - Heading resolution: the heading part matches by heading **text** (not full heading-path
    breadcrumb) within the resolved file. Zero or multiple matching headings is "unresolved" with
    the candidates listed, mirroring the existing `RetrievalError` suggestions pattern.
- **Vault-wide link index** built alongside section indexing: every `WikiLink` inline found while
  walking each document's sections is recorded as one *link record* — its containing section
  (`heading_path`), its raw target text, and whether it resolved (and to what).
- New MCP tool: `backlinks(file, heading_path?)` — every indexed link whose target resolves to
  that file (optionally narrowed to one section), with the linking section's provenance
  (including `content_hash`, consistent with every other read result since the section-editing
  change).
- Unresolved links are surfaced as non-fatal `VaultDiagnostic` entries (reusing the existing
  diagnostic channel), not a separate error path.

**Deferred to follow-up changes** (decided during exploration): fixing wikilinks as part of a
future `rename_section`/`delete_section` (needs cross-file atomic writes, a bigger primitive than
anything built so far); an outbound `links_from` tool (an agent can already see raw wikilink text
in `get_section`'s content); resolving wikilinks that point at other Markdown-like conventions
(e.g. an Obsidian block-reference `^id` suffix) — out of scope until requested.

## Capabilities

### New Capabilities
- `wikilink-graph`: Engine-level resolution of `[[target]]` / `[[target#Heading]]` /
  `[[#Heading]]` wikilinks to sections, a vault-wide backlink index, and the `backlinks` MCP tool.

### Modified Capabilities
(none — `outline`, `get_section`, `search`, and `edit_section` behavior and response shapes are
unchanged; `section-editing`'s `Provenance.content_hash` is reused as-is by `backlinks` results)

## Impact

- `crates/context-engine`: new link-index module (link record type, target-split, resolution),
  wired into `VaultIndex::build` and `VaultIndex::reindex_file` (backlinks must stay correct
  after a section edit); new unit + integration tests.
- `src/server.rs` / `src/main.rs`: new `backlinks` tool, read-locked like the other read tools.
- `crates/context-lexer`, `crates/context-parser`: untouched — the opaque `WikiLink.target` span
  is exactly what the engine needs; no parser change required.
- Fixture vault gains wikilink examples (resolved, unresolved-file, unresolved-heading, ambiguous,
  self-link) to exercise the new resolution rules end-to-end.
- Changelogs: `context-engine` (MINOR — new read API) and root binary per convention; `CLAUDE.md`
  updated to list the fifth tool.
