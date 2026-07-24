## Why

The engine is strictly read-only: an agent can find and read `Skills > Gun` exactly, but to change
it must rewrite the whole file through some other tool — losing every guarantee the engine exists
to provide (byte-exact spans, untouched surroundings, conflict detection). The foundation change
already paid for the write path without using it: every section carries an absolute source span
(splice target) and a BLAKE3 `content_hash` of its exact bytes (a ready-made optimistic-concurrency
token). This change adds the smallest safe write primitive on top of that groundwork: replace one
section's body, guarded by its hash, leaving every other byte of the file untouched.

## What Changes

- New `context-engine` write API: replace the **body** of a section (heading line preserved)
  addressed by file + heading path, guarded by the section's `content_hash`:
  - The hash is verified against **fresh disk bytes** (the file is re-read and re-derived at write
    time), not the in-memory index — disk is the source of truth, so a stale index can never
    corrupt a write. Mismatch is a conflict error, no write occurs.
  - New body must parse standalone with no heading of level ≤ the target section's level
    (rejecting "section escape" — an edit may not restructure the document).
  - Boundary discipline: the splice replaces exactly the body's byte range; trailing
    whitespace-only lines are dropped from the new body, interior content is written verbatim, and
    all bytes outside that range — including inter-section separators — are preserved
    byte-identically. A post-splice structural verification re-parses the result and rejects the
    edit if any section outside the edited one changed shape.
  - Atomic write: temp file + rename in the vault; the edited document is re-parsed into the index
    after the write.
- Read-path hash provisioning: `Provenance` (returned by `get_section` and `search`) gains a
  `content_hash` field so an agent can go straight from read to guarded edit without an extra
  round-trip.
- New MCP tool on the `context` binary: `edit_section(file, heading_path, body, expected_hash)`,
  returning the edited document's fresh outline **with new section hashes**, so the agent
  immediately holds valid tokens for a follow-up edit.
- `VaultIndex` becomes mutable behind the server: a single-document reindex method on the engine,
  and the server wraps the index for interior mutability (write lock during edit+reindex).

**Deferred to follow-up changes** (decided during exploration): `insert_section` /
`delete_section` / `rename_section` (rename possibly with wikilink fix-up), any document
restructuring ops, and file watching for read-path staleness.

## Capabilities

### New Capabilities
- `section-editing`: Hash-guarded, body-only section replacement — disk-verified optimistic
  concurrency, section-escape rejection, boundary normalization, atomic write, single-document
  reindex — exposed as the `edit_section` MCP tool.

### Modified Capabilities
- `structural-retrieval` (additively, specified within `section-editing`): retrieval and search
  provenance gains `content_hash`. Everything else about `outline`, `get_section`, and `search`
  is unchanged; `mcp-server` gains no new requirements — the new tool is specified under
  `section-editing`.

## Impact

- `crates/context-engine`: new write module (edit + normalization + validation), a `&mut self`
  single-file reindex on `VaultIndex`; new unit tests. No public read-API changes.
- `src/server.rs` / `src/main.rs`: `edit_section` tool; index moves behind a lock
  (read tools take read guards, edit takes the write guard).
- `crates/context-lexer`, `crates/context-parser`: untouched.
- Changelogs: `context-engine` (MINOR — new API) and root binary per convention; `CLAUDE.md`
  updated to list the fourth tool and drop "read-only" phrasing.
