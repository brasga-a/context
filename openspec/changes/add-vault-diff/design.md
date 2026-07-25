## Context

`VaultIndex::build(root)` (`crates/context-engine/src/vault.rs`) is a complete, self-contained,
side-effect-free vault walk: given a root path it returns a fresh `VaultIndex` or a `VaultError`,
touching nothing but its own locals along the way. Every section already carries a BLAKE3
`content_hash` over its exact span bytes (added in the section-editing change as an
optimistic-concurrency token, reused unchanged here). `VaultIndex::edit_section` already calls a
private `reindex_file` after every successful write, but only to replace the in-memory document —
the discarded old tree is never inspected. No tool exposes any form of "re-read the vault" to an
agent today; the only way the in-memory index changes is a full server restart, or a write made
through `edit_section` itself. This change was scoped in the 2026-07-25 exploration session,
which also reframed the original foundation design's "`changes_since` diffs" item: that design
bundled diffing with LanceDB persistence and embeddings, but diffing needs neither — it only needs
the "before" and "after" `VaultIndex` values, both of which already exist for one instant at the
point `reindex_file`/`edit_section` currently throw the old one away.

## Goals / Non-Goals

**Goals:**
- `VaultIndex::reindex_vault(&mut self) -> Result<VaultDiff, VaultError>`: re-walk the vault root
  via `VaultIndex::build`, diff the result against `self`, swap to it, return the diff.
- File-level diff: added / removed files.
- Section-level diff (within files present before and after): added / removed / modified heading
  paths, using each section's existing `content_hash`.
- A `reindex_vault` MCP tool exposing this to agents — the first tool that lets an agent ask the
  server to look at disk again.
- Deterministic, low-noise reporting: each side of the diff reports the *root cause* of a change,
  not every path that merely inherited a changed hash or sits under a new/removed subtree.

**Non-Goals:**
- No file watching or automatic triggering — this is a manually invoked tool. It produces exactly
  the diff payload a future watcher would need, but the watcher itself (deciding *when* to call
  this) is separate follow-up work.
- No cross-restart history and no persistence of any kind. The "before" state is `self` at the
  moment of the call; nothing survives a server restart, matching every other read-path behavior
  in this codebase.
- No per-section "own content only" hash. `content_hash` stays exactly what it is today (whole
  subtree). The deepest-changed-only heuristic below is an accepted approximation, not a precise
  content-change detector.
- No rename detection, at either file or heading granularity. A renamed file is reported as one
  path removed and a different path added; a renamed heading is reported the same way at the
  section level. This mirrors the foundation change's existing decision that heading-path identity
  treats renames as delete + add.
- No diffing of `VaultIndex.diagnostics` (e.g. a previously-broken wikilink that now resolves).
  Diagnostics are already visible via the field after the call; surfacing *changes* to them is a
  possible future enhancement, not required here.

## Decisions

### 1. `reindex_vault` is `build` + diff + swap, nothing else
```rust
pub fn reindex_vault(&mut self) -> Result<VaultDiff, VaultError> {
    let new_index = VaultIndex::build(&self.root)?;
    let diff = diff_vaults(self, &new_index);
    *self = new_index;
    Ok(diff)
}
```
No new directory-walking code, no new error type — `VaultError` (already returned by `build`) is
reused as-is. If the root is gone or not a directory, `build` fails before `self` is touched: no
diff, no swap, no false "everything was removed" report.
- **Alternative considered**: incrementally re-stat only files that changed (mtime-based) —
  rejected; `build` already parses a full vault in milliseconds (established in the foundation
  change), and a full re-walk is trivially correct where incremental tracking would need its own
  new invalidation logic to get right.

### 2. Section-level diff flattens each file's tree, no recursive tree-walk needed
Both the old and new section trees for a file are flattened into `heading_path -> content_hash`
maps via the existing `visit_sections` walker (already used by `find_section`'s siblings in
`vault.rs`). Added/removed/candidate-modified sets fall out of simple key-set comparison — no
structural tree-diffing algorithm is needed because `heading_path` is already a globally unique,
flat identity within a file.

### 3. Report the root cause, not its echoes — opposite directions for additions and modifications
`content_hash` covers a section's full subtree, so a single edit produces cascading hash changes
up the ancestor chain, and adding a new subtree produces a "new heading" for every node in it.
Reporting every echo is noise. The engine reports only the root cause, but "root" points in
opposite directions depending on the kind of change:
- **`sections_modified`**: report a path only if **no** other changed path (added, removed, *or*
  modified) is nested under it. A parent's hash changing solely because a child changed is pure
  cascade — the child is the real root cause, reported instead.
- **`sections_added`** / **`sections_removed`**: report a path only if its *parent* is not also
  in the same added/removed set. Adding `## Notes` with a new child `### Sub` reports only
  `Notes` — `Sub` comes along for free as part of the new subtree an agent can see by reading the
  file's outline; deleting `## Skills` along with everything under it reports only `Skills`.

Both rules reduce to the same shape: within one file, suppress a changed path if some *other*
changed path stands in an ancestor/descendant relationship with it, in the direction that makes
that other path the more useful one to surface. The check itself is a plain string-prefix test
(`candidate.starts_with(&format!("{other} > "))`) over the small per-file change set — no tree
structure needs to be walked a second time.
- **Alternative considered**: report every changed path unfiltered — rejected; for a body edit
  three levels deep, an agent would see four "changed" entries (the edited section and three
  ancestors) for one actual edit, with no way to tell which one is real without re-reading all
  four.
- **Known limitation, accepted**: if a section's own body *and* one of its children both changed
  in the same reindex window, `sections_modified` reports only the child — the parent's own edit
  is indistinguishable from pure cascade with a whole-subtree hash. Precise disambiguation would
  need a second, narrower hash (Non-Goals). An agent that wants certainty about a specific
  section can always follow up with `get_section` and compare against its own last-known hash.

### 4. Renames are delete + add at both granularities, deliberately
Already decided for headings in the foundation change; this change applies the same reasoning to
files. No path-similarity heuristics, no "this looks like the old file moved" guessing — a rename
is exactly as informative as its two halves (old path removed, new path added) and nothing here
tries to reunite them.

### 5. Empty-diff files are omitted; the tool result stays deterministic
A file present both before and after with no added/removed/modified sections after the Decision 3
filtering does not appear in `files_changed` at all. All lists (`files_added`, `files_removed`,
each file's three section lists) are sorted for deterministic output, matching every other
list-returning tool in this codebase.

### 6. Locking: same discipline as `edit_section`, no new pattern
The MCP `reindex_vault` tool takes the server's write lock across the whole
build-diff-swap, exactly like `edit_section` already does. `reindex_vault` itself has no partial-
failure state to guard against: `self` is only ever swapped *after* the new index has been fully
built and diffed, so a failure always leaves `self` exactly as it was.

## Risks / Trade-offs

- **Full re-walk cost on every call** — accepted; same cost class as startup `build`, already
  established as millisecond-scale for the vaults this engine targets.
- **Deepest/shallowest-only filtering can hide a real edit behind a cascade** (Decision 3's known
  limitation) — accepted, with the `get_section` fallback path noted above.
- **Diagnostics changes are invisible to the diff** — accepted as a Non-Goal; visible via
  `VaultIndex.diagnostics` after the call regardless.

## Migration

No data or API migration. Existing tools and their responses are unchanged. New engine API and
MCP tool are additive (MINOR).

## Open Questions

(none — scope was narrowed to vault-wide, manually-triggered, non-persistent diffing during
exploration; file watching and a finer-grained hash are named follow-ups, not open questions)
