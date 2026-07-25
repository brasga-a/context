## 1. Diff computation (context-engine)

- [x] 1.1 Define `VaultDiff { files_added, files_removed, files_changed: Vec<FileDiff> }` and
      `FileDiff { file, sections_added, sections_removed, sections_modified }` (all lists
      `Vec<String>` of vault-relative paths / heading paths, sorted); `Serialize` for MCP
      responses
- [x] 1.2 Implement per-file flattening: `heading_path -> content_hash` map via the existing
      `visit_sections` walker; unit tests against fixture documents (flat, nested, preamble)
- [x] 1.3 Implement `diff_vaults(old: &VaultIndex, new: &VaultIndex) -> VaultDiff`: file-level
      added/removed from `documents` key-set difference; per file present in both, section-level
      added/removed/modified-candidates from flattened-map key/value comparison; unit tests for
      each category in isolation
- [x] 1.4 Implement root-cause filtering: suppress a modified path with a changed (added/
      removed/modified) descendant; suppress an added path whose parent is also added; suppress a
      removed path whose parent is also removed; unit tests per spec scenarios (cascading child
      edit, new subtree, removed subtree) plus a case with no filtering needed
- [x] 1.5 Implement file-level and heading-level rename-is-delete-plus-add behavior (this falls
      out of 1.3/1.4 with no special-casing — add a regression test asserting no rename detection
      occurs, matching the spec's explicit non-goal)
- [x] 1.6 Implement `VaultIndex::reindex_vault(&mut self) -> Result<VaultDiff, VaultError>`:
      `VaultIndex::build(&self.root)`, diff against `self`, swap, return diff; fails without
      diffing or swapping if `build` fails; unit/integration tests: successful reindex reflects
      new disk content, missing root fails and leaves the old index and its diagnostics/backlinks
      untouched, unchanged file absent from `files_changed`

## 2. MCP tool (context binary)

- [x] 2.1 Implement `reindex_vault` tool (no parameters) on the write lock, mirroring
      `edit_section`'s locking; success returns the `VaultDiff`; failure surfaces as a tool error
      naming the root path
- [x] 2.2 End-to-end verification over stdio against a temp vault: reindex after an external file
      add, an external file removal, an external section edit, an external section addition
      nested under a new heading, and a no-op reindex (empty diff); confirm subsequent
      `get_section`/`outline`/`backlinks` calls reflect the new state

## 3. Wrap-up

- [x] 3.1 Update `CLAUDE.md`: sixth tool `reindex_vault`, note it is the only tool that re-reads
      disk outside of a write, and the deepest/shallowest-only reporting convention
- [x] 3.2 Update `README.md`: add `reindex_vault` to the MCP tools table; new short section
      explaining when to call it (picking up changes made outside the server) and the accepted
      cascade-hiding limitation
- [x] 3.3 Update changelogs per convention: `context-engine` (**MINOR** — new API), root binary
      changelog
- [x] 3.4 Run `/check` across the workspace; leave formatted, lint-clean, all tests passing
