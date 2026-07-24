## ADDED Requirements

### Requirement: Hash-guarded body replacement
The engine SHALL replace the body of a section addressed by file path plus heading path, preserving
the heading line, only when the caller-supplied expected hash equals the BLAKE3 content hash of the
section as re-derived from the file's current on-disk bytes at write time. On hash mismatch the
engine SHALL fail with a conflict error carrying the current hash and SHALL NOT modify the file.
All bytes outside the edited section's body SHALL remain byte-identical.

#### Scenario: Matching hash edits exactly one section body
- **WHEN** `edit_section` is called for `player.md` + `Skills > Gun` with the section's current
  content hash and a new body
- **THEN** the file on disk contains the unchanged heading line followed by the new body, and
  every byte outside that section's body is identical to before the edit

#### Scenario: Stale hash is a conflict, file untouched
- **WHEN** `edit_section` is called with a hash that does not match the section's current on-disk
  bytes
- **THEN** the result is a conflict error that includes the section's current hash, and the file
  is byte-identical to before the call

#### Scenario: Disk trumps a stale index
- **WHEN** the file was modified on disk after indexing and `edit_section` is called with the hash
  matching the *current* disk content
- **THEN** the edit succeeds against the current disk content

#### Scenario: Unknown file or heading path fails helpfully
- **WHEN** `edit_section` names a file or heading path that does not exist on disk
- **THEN** the result is a not-found error naming what was missing, with nearest-match
  suggestions when available, and no file is modified

### Requirement: Section escape rejection
The engine SHALL parse the new body standalone and reject the edit if it contains any heading of
level less than or equal to the target section's level, naming the offending heading and its
level. Headings of deeper level SHALL be accepted as child sections.

#### Scenario: Body containing an equal-level heading is rejected
- **WHEN** the target section is level 2 and the new body contains a level-2 (or level-1) heading
- **THEN** the edit fails with an error naming that heading and its level, and the file is
  unmodified

#### Scenario: Deeper child headings are accepted
- **WHEN** the target section is level 2 and the new body contains only level-3+ headings
- **THEN** the edit succeeds and the new child sections appear nested under the edited section

### Requirement: Span-tight splice and structural verification
The engine SHALL write the interior of the new body verbatim, dropping only trailing
whitespace-only lines so the written body ends at a content byte, and SHALL preserve all bytes
outside the replaced body range — including inter-section separators — byte-identically. An empty
body SHALL be legal and leave only the heading line. Before writing, the engine SHALL re-parse
the spliced document and reject the edit (without writing) if any section outside the edited one
changed heading path or level.

#### Scenario: Trailing whitespace is trimmed and sections stay separate
- **WHEN** the new body ends with trailing blank lines or lacks a trailing newline, and another
  section follows the edited one
- **THEN** the written body ends at its last content byte, the original separator bytes before
  the next heading are preserved unchanged, and both sections parse as before

#### Scenario: Interior content is verbatim
- **WHEN** the new body contains a code block with trailing spaces and blank lines inside it
- **THEN** those interior bytes appear in the file unchanged

#### Scenario: A splice that would merge into a following section is rejected
- **WHEN** the new body passes the standalone heading check but the spliced document would change
  the heading path or level of any section outside the edited one (e.g. a trailing paragraph
  gluing into a following setext underline)
- **THEN** the edit fails with a restructure error and the file is unmodified

### Requirement: Atomic write and index freshness
The engine SHALL write edits atomically (a partially written file SHALL never be observable at the
target path) and SHALL re-derive the edited document's section tree from the written content
before reporting success. On any failure the original file SHALL remain intact and the index
unchanged.

#### Scenario: Failed edit leaves no trace
- **WHEN** an edit fails validation at any stage (conflict, escape, unknown path)
- **THEN** the target file is byte-identical to before the call and subsequent reads reflect the
  pre-edit index

#### Scenario: Successful edit is immediately readable
- **WHEN** an edit succeeds and `get_section` is then called for the same heading path
- **THEN** the returned content is the normalized new body under the preserved heading

### Requirement: Read-path hash provisioning
Retrieval and search provenance SHALL include the section's current content hash so a caller can
proceed from any read result to a guarded edit without additional lookups. This addition SHALL be
the only change to read-path responses.

#### Scenario: get_section supplies the edit token
- **WHEN** `get_section` returns a section
- **THEN** its provenance includes the section's content hash, and calling `edit_section` with
  that hash (against an unchanged file) succeeds

#### Scenario: Search results supply edit tokens
- **WHEN** `search` returns results
- **THEN** each result's provenance includes that section's content hash

### Requirement: edit_section MCP tool
The `context` MCP server SHALL expose an `edit_section` tool taking file, heading path, new body,
and expected hash, mapped onto the engine's guarded replacement. Success SHALL return the edited
document's outline with each section's fresh content hash; failures SHALL surface the engine's
conflict / escape / not-found errors as tool errors with their messages and suggestions. Existing
read tools SHALL remain unchanged in behavior and response shape, and concurrent read tool calls
SHALL never observe a partially applied edit.

#### Scenario: Success returns fresh hashes
- **WHEN** `edit_section` succeeds
- **THEN** the response contains the document's outline with the new content hash for every
  section, sufficient to issue a follow-up edit without re-reading

#### Scenario: Conflict surfaces as a tool error with the current hash
- **WHEN** `edit_section` is called with a stale hash
- **THEN** the tool returns an error result whose message states the conflict and includes the
  section's current hash
