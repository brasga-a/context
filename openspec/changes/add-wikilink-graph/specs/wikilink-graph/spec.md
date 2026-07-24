## ADDED Requirements

### Requirement: Wikilink target parsing
The engine SHALL interpret a wikilink's target text by splitting it on its first `#` character:
the substring before is the file part, the substring after (if any) is the heading part. A target
with no `#` SHALL be a whole-file reference with no heading part. A target whose file part is
empty (e.g. `#Heading`) SHALL refer to the file containing the link.

#### Scenario: Whole-file target has no heading part
- **WHEN** a wikilink's target text is `Weapons` (no `#`)
- **THEN** the file part is `Weapons` and the heading part is absent

#### Scenario: File-and-heading target splits on the first hash
- **WHEN** a wikilink's target text is `Weapons#Gun`
- **THEN** the file part is `Weapons` and the heading part is `Gun`

#### Scenario: Empty file part means the current file
- **WHEN** a wikilink's target text is `#Gun` and it appears in `player.md`
- **THEN** the file part resolves to `player.md`

### Requirement: File resolution
The engine SHALL resolve a non-empty file part to exactly one indexed file: a file part
containing no `/` SHALL match by filename stem (the path segment with `.md` removed) across every
indexed file vault-wide; a file part containing `/` SHALL match as a full vault-relative path
(with or without a trailing `.md`). Zero matches or more than one match SHALL be unresolved rather
than a build error.

#### Scenario: Stem match finds a file in any directory
- **WHEN** the vault contains `lore/weapons.md` and a wikilink's file part is `weapons`
- **THEN** the file part resolves to `lore/weapons.md`

#### Scenario: No matching file is unresolved
- **WHEN** a wikilink's file part matches no indexed file
- **THEN** the link is unresolved and the vault indexes successfully regardless

#### Scenario: Ambiguous stem is unresolved
- **WHEN** two indexed files share the same filename stem in different directories and a
  wikilink's file part matches that stem
- **THEN** the link is unresolved

### Requirement: Heading resolution within a resolved file
Given a resolved file and a non-empty heading part, the engine SHALL resolve to exactly one
section whose heading text equals the heading part. Zero matches or more than one match (e.g. two
sections sharing heading text, disambiguated internally) SHALL be unresolved, and when more than
one section matches, the candidate heading paths SHALL be discoverable the same way
`get_section`'s not-found suggestions are.

#### Scenario: Unique heading text resolves
- **WHEN** the resolved file has exactly one section with heading text `Gun`
- **THEN** the heading part `Gun` resolves to that section's heading path

#### Scenario: Duplicate heading text is unresolved with candidates
- **WHEN** the resolved file has two sections with heading text `Gun` (heading paths
  `Skills > Gun` and `Skills > Gun[2]`) and the heading part is `Gun`
- **THEN** the link is unresolved and both heading paths are available as candidates

### Requirement: Vault-wide backlink index
The engine SHALL maintain a backlink index mapping each file to every resolved wikilink pointing
at it, each carrying the linking section's full provenance (including `content_hash`), the link's
raw target text, and the resolved target heading path when the link had a heading part. The index
SHALL be correct across the whole vault after `VaultIndex::build` and after every successful
`edit_section` or `reindex_file` call, including when a change to one file alters whether a link
in a different, unchanged file resolves.

#### Scenario: A link is indexed as a backlink of its resolved target
- **WHEN** `player.md`'s `Skills > Gun` section contains `[[Weapons#Gun Skill]]` which resolves
- **THEN** the backlink index for the resolved target file includes an entry whose provenance is
  `player.md`'s `Skills > Gun` section

#### Scenario: Editing one file updates another file's link resolution
- **WHEN** file A has no heading named `Widget`, file B contains `[[A#Widget]]` (unresolved), and
  an `edit_section` call on file A adds a child section headed `Widget`
- **THEN** after the edit, B's link to `A#Widget` appears in the backlink index for A without B
  having been re-read or re-edited

### Requirement: backlinks MCP tool
The `context` MCP server SHALL expose a `backlinks` tool taking a file and an optional heading
path, returning every indexed backlink for that file, narrowed to the given heading path when one
is supplied. The tool SHALL share the read lock used by `outline`, `get_section`, and `search`.

#### Scenario: Backlinks for a whole file
- **WHEN** `backlinks` is called with only a file
- **THEN** the response includes every resolved link pointing anywhere in that file

#### Scenario: Backlinks narrowed to one heading path
- **WHEN** `backlinks` is called with a file and a heading path
- **THEN** the response includes only links whose resolved target heading path matches

### Requirement: Unresolved links are non-fatal diagnostics
An unresolved wikilink (no file match, ambiguous file match, no heading match, or ambiguous
heading match) SHALL be recorded as a `VaultDiagnostic` naming the source file, the linking
section, and the unresolved target text. It SHALL NOT fail vault indexing or any edit.

#### Scenario: An unresolved link surfaces as a diagnostic, not a failure
- **WHEN** the vault contains a wikilink whose target does not resolve
- **THEN** `VaultIndex::build` succeeds and its diagnostics include an entry naming the source
  file and the unresolved target
