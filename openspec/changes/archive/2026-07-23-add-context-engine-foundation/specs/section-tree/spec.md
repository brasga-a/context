## ADDED Requirements

### Requirement: Section tree derivation from parsed documents
The engine SHALL derive a nested section tree from a parsed document's flat block stream, where a
section consists of one heading plus all following sibling blocks until the next heading of
equal-or-higher level, and headings of lower level nest as child sections.

#### Scenario: Nested sections from heading levels
- **WHEN** a document contains `# Player`, then `## Skills`, then `### Gun` with content, then
  `### Sword` with content, then `## Inventory`
- **THEN** the tree has `Player` containing `Skills` and `Inventory`, with `Skills` containing
  `Gun` and `Sword` as child sections

#### Scenario: Content before the first heading
- **WHEN** a document has paragraphs before its first heading
- **THEN** that content is grouped into a synthetic preamble section at the root of the tree

#### Scenario: Skipped heading levels
- **WHEN** a document jumps from `#` directly to `###` with no `##` between
- **THEN** the `###` section nests directly under the `#` section without synthetic intermediate
  levels

### Requirement: Sections are span-backed
Every section SHALL record the absolute byte range in the source file covering its heading and all
contained blocks, such that slicing the source with that range reproduces the section's exact
original text, and child section spans SHALL lie within their parent's span.

#### Scenario: Section span reproduces source text
- **WHEN** a section's span is used to slice the original source string
- **THEN** the result is exactly the section's heading line plus its content as written, byte for
  byte

### Requirement: Heading-path identity
Every section SHALL have a heading-path identity: the sequence of heading texts from the root to
that section (e.g. `Skills > Gun`), unique within its file. Duplicate sibling headings SHALL be
disambiguated deterministically with an index suffix.

#### Scenario: Unique paths for duplicate headings
- **WHEN** a document contains two sibling sections both titled `## Notes`
- **THEN** their paths are distinct (e.g. `Notes` and `Notes[2]`) and stable across re-parses of
  the same content

### Requirement: Section content hashing
Every section SHALL carry a content hash computed over the exact source bytes of its span, such
that two sections with byte-identical content have equal hashes and any byte change in a section's
content changes its hash.

#### Scenario: Hash stability and sensitivity
- **WHEN** a document is re-parsed without modification, and separately re-parsed after editing
  one section's text
- **THEN** unmodified sections keep identical hashes and the edited section's hash changes

### Requirement: Frontmatter parsed as document metadata
The engine SHALL parse the frontmatter span (when present and terminated) as YAML and expose the
result as the document's metadata. Invalid YAML SHALL NOT fail document indexing; it SHALL be
recorded as a diagnostic with metadata treated as absent.

#### Scenario: Valid frontmatter becomes metadata
- **WHEN** a document begins with `---`, `tags: [character]`, `---`
- **THEN** the document's metadata contains `tags: [character]`

#### Scenario: Invalid frontmatter is tolerated
- **WHEN** a document's frontmatter is not valid YAML
- **THEN** the document still indexes with its sections intact, metadata is absent, and a
  diagnostic records the YAML error
