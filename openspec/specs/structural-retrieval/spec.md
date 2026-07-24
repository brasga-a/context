## Purpose

Defines vault indexing and deterministic, provenance-rich retrieval of document structure and
section content.

## Requirements

### Requirement: Vault indexing
The engine SHALL index a vault (a directory tree of markdown files), parsing each `.md` file into
its section tree and making all sections addressable by file path and heading path. A file that
fails to read SHALL be skipped with a recorded diagnostic without failing the vault index.

#### Scenario: Directory of markdown files is indexed
- **WHEN** the engine indexes a vault directory containing `.md` files in nested subdirectories
- **THEN** every markdown file's sections are addressable by that file's vault-relative path

#### Scenario: Non-markdown files are ignored
- **WHEN** the vault directory also contains non-`.md` files
- **THEN** they are not indexed and produce no errors

### Requirement: Document outline
The engine SHALL produce an outline for any indexed file: its section tree as heading text, level,
heading path, and source line range per section, without section body content.

#### Scenario: Outline reflects the section tree
- **WHEN** the outline of an indexed file is requested
- **THEN** it lists every section in document order with heading text, level, full heading path,
  and line range, and contains no body text

### Requirement: Exact section retrieval
The engine SHALL resolve a file path plus heading path to the exact source slice of that section,
including provenance (file, heading path, byte range, line range). An unknown file or heading path
SHALL produce a not-found error naming what was missing, with nearest-match suggestions when
available.

#### Scenario: Known section returns exact source
- **WHEN** `player.md` + heading path `Skills > Gun` is requested
- **THEN** the response contains the byte-exact source of that section and its provenance

#### Scenario: Unknown heading path fails helpfully
- **WHEN** a heading path that does not exist in the file is requested
- **THEN** the result is a not-found error that names the missing path and suggests the closest
  existing heading paths in that file

### Requirement: Fuzzy heading search across the vault
The engine SHALL answer free-text queries by matching against heading texts and heading paths
across all indexed files, case-insensitively and tolerant of word-order and partial-token
differences, returning ranked results with provenance. Exact matches SHALL rank above partial
matches, and ranking SHALL be deterministic for identical inputs.

#### Scenario: Query matches a heading despite inexact phrasing
- **WHEN** the vault contains `## Gun Skill` in `player.md` and the query is `gun skill` or
  `skill gun` or `Gun`
- **THEN** results include that section, with the more exact query forms ranking it at least as
  high as the less exact ones

#### Scenario: Results carry provenance
- **WHEN** any search returns results
- **THEN** each result includes file path, full heading path, and line range, sufficient to call
  exact section retrieval without further lookup

#### Scenario: Deterministic ranking
- **WHEN** the same query is run twice against an unchanged vault
- **THEN** the results and their order are identical
