## Why

The engine indexes a vault once at startup and never looks at disk again on its own; the only way
its in-memory copy of a file gets refreshed is `VaultIndex::reindex_file`, called internally by
`edit_section` right after it writes — and even then, the old section tree is simply discarded,
never compared against the new one. An agent has no way to ask "did anything outside my own edits
change since I started" — not a human editing the vault in Obsidian, not another process, not a
git checkout. The original foundation design bundled this need ("`changes_since` diffs") together
with LanceDB persistence and embeddings as one deferred bucket, but re-reading the code shows the
coupling was incidental: `VaultIndex::build(root)` is already a complete, self-contained,
side-effect-free rebuild. Diffing the vault requires no persistence at all — only comparing two
in-memory snapshots that already both exist for one instant, right where the old one would
otherwise be thrown away.

## What Changes

- New `context-engine` capability: `VaultIndex::reindex_vault(&mut self)` re-walks the vault root
  via the existing `VaultIndex::build` path, computes a diff against the index being replaced,
  swaps to the fresh index, and returns the diff. No new storage, no snapshot concept, no
  timestamp bookkeeping — the "before" state is simply `self` at the moment of the call.
- Diff shape, at file and section granularity:
  - Files added (new `.md` files found), files removed (previously indexed files no longer
    found).
  - For every file present before and after: sections added, sections removed, and sections
    modified (by heading path, using the section's existing `content_hash`).
  - A file with no section-level differences is omitted from the report entirely.
  - Section-level reporting uses "deepest-changed-only": a heading path is suppressed from
    `modified` if some path nested under it also changed, since `content_hash` covers a section's
    full subtree and a child's edit necessarily changes every ancestor's hash too. This is a
    documented, accepted imprecision (see design.md) — the engine has no per-section "own content
    only" hash to disambiguate a genuine parent-level edit that happens to coincide with a child
    edit in the same reindex window.
  - A renamed heading is reported as its old path removed and its new path added — never as a
    "rename" — consistent with the foundation change's existing decision that heading-path
    identity treats renames as delete + add.
- New MCP tool: `reindex_vault()` (no parameters), returning the diff. Takes the write lock, same
  discipline as `edit_section`.
- Fails fast (no diff, no swap) if the vault root itself is no longer a valid directory, mirroring
  `VaultIndex::build`'s existing error behavior — a missing root must never be reported as "every
  file was removed."

**Deferred to follow-up changes** (decided during exploration): file watching / automatic
reindexing (this change adds the manual trigger and the diff payload a future watcher would reuse
verbatim — the watcher is purely "call this on a filesystem event" once it exists); a
finer-grained "own content changed" hash to remove the deepest-changed-only imprecision;
persistent cross-restart history (LanceDB) — this change's diff only ever compares "before this
call" to "after this call," nothing survives a server restart.

## Capabilities

### New Capabilities
- `vault-diff`: Re-walking the vault and reporting what changed (files and sections, added /
  removed / modified) since the previous in-memory index, exposed as the `reindex_vault` MCP
  tool.

### Modified Capabilities
(none — `outline`, `get_section`, `search`, `edit_section`, and `backlinks` are unchanged; the
existing internal `reindex_file` is untouched, this change adds a sibling vault-wide operation
rather than modifying it)

## Impact

- `crates/context-engine`: new diff module (flatten-and-compare over `documents`, deepest-changed
  filtering), `VaultIndex::reindex_vault`; new unit + integration tests.
- `src/server.rs`: new `reindex_vault` tool, read/write lock reused as-is.
- `crates/context-lexer`, `crates/context-parser`: untouched.
- Changelogs: `context-engine` (MINOR — new API) and root binary per convention; `CLAUDE.md` and
  `README.md` updated to list the sixth tool.
