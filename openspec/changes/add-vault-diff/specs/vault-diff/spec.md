## ADDED Requirements

### Requirement: Vault-wide reindex with fail-fast on a bad root
The engine SHALL re-walk the vault root, build a fresh index, and replace its current index with
the fresh one, returning a diff between the replaced index and the fresh one. If the vault root is
no longer a valid directory, the engine SHALL fail without producing a diff and without replacing
the current index.

#### Scenario: A successful reindex replaces the index and returns a diff
- **WHEN** `reindex_vault` is called and the vault root is still a valid directory
- **THEN** the index now reflects the vault root's current on-disk content, and a diff describing
  what changed since the previous index is returned

#### Scenario: A missing root fails without a false "everything removed" diff
- **WHEN** `reindex_vault` is called and the vault root no longer exists or is not a directory
- **THEN** the call fails, the previous index is left in place, and no diff is produced

### Requirement: File-level diff
The diff SHALL report every vault-relative file path present after the reindex but not before as
added, and every path present before but not after as removed.

#### Scenario: A new file is reported as added
- **WHEN** a `.md` file exists on disk that was not in the previous index
- **THEN** its vault-relative path appears in the diff's added files

#### Scenario: A deleted file is reported as removed
- **WHEN** a previously indexed file no longer exists on disk
- **THEN** its vault-relative path appears in the diff's removed files

### Requirement: Section-level diff within unchanged files
For every file present both before and after the reindex, the diff SHALL report which heading
paths are new, which are gone, and which have a different `content_hash` than before. A file with
no such differences SHALL NOT appear in the diff's changed-files list.

#### Scenario: A new section within an existing file is reported
- **WHEN** a file gains a new heading that did not exist in the previous index
- **THEN** its heading path appears among that file's added sections

#### Scenario: A removed section within an existing file is reported
- **WHEN** a heading present in the previous index no longer exists in the file
- **THEN** its heading path appears among that file's removed sections

#### Scenario: An edited section is reported as modified
- **WHEN** a section's content changed such that its `content_hash` differs from the previous
  index's value for that heading path
- **THEN** its heading path appears among that file's modified sections

#### Scenario: An unchanged file is absent from the diff
- **WHEN** a file's section set and every section's `content_hash` are identical before and after
- **THEN** that file does not appear in the diff's changed-files list

### Requirement: Root-cause filtering suppresses cascading and redundant entries
Because a section's `content_hash` covers its full subtree, the diff SHALL suppress a modified
heading path when some other changed path (added, removed, or modified) is nested under it. The
diff SHALL suppress an added heading path when its parent path is also newly added, and SHALL
suppress a removed heading path when its parent path is also removed.

#### Scenario: A child edit does not also report every ancestor as modified
- **WHEN** a deeply nested section's body changes and no other section in the file changes
- **THEN** only that section's heading path appears in modified sections, not any of its ancestors

#### Scenario: A new subtree reports only its topmost new heading
- **WHEN** a new heading is added along with new child headings beneath it, none of which existed
  before
- **THEN** only the topmost new heading path appears in added sections

#### Scenario: A removed subtree reports only its topmost removed heading
- **WHEN** a heading and everything nested beneath it are removed together
- **THEN** only the topmost removed heading path appears in removed sections

### Requirement: Renames are delete plus add, at both file and heading granularity
The diff SHALL NOT attempt to detect renames. A file renamed on disk SHALL be reported as its old
path removed and its new path added. A heading renamed within an otherwise-unchanged file SHALL be
reported as its old heading path removed and its new heading path added.

#### Scenario: A renamed file appears as removed plus added
- **WHEN** a previously indexed file's path changes on disk with unchanged content
- **THEN** the diff reports the old path as removed and the new path as added, with no indication
  the two are related

#### Scenario: A renamed heading appears as removed plus added within its file
- **WHEN** a section's heading text changes but its body does not
- **THEN** the diff reports the old heading path as removed and the new heading path as added for
  that file

### Requirement: reindex_vault MCP tool
The `context` MCP server SHALL expose a `reindex_vault` tool taking no parameters, mapped onto the
engine's vault-wide reindex. It SHALL take the same write lock used by `edit_section`, serializing
against concurrent reads and writes. Success SHALL return the diff; failure (an invalid vault
root) SHALL surface as a tool error naming the root path.

#### Scenario: A successful call returns the diff
- **WHEN** `reindex_vault` is called with the vault root intact
- **THEN** the response contains the added/removed files and, per changed file, its added/
  removed/modified section heading paths

#### Scenario: An invalid root surfaces as a tool error
- **WHEN** `reindex_vault` is called and the vault root is invalid
- **THEN** the tool returns an error naming the root path, and subsequent read tools continue to
  serve the unchanged previous index
