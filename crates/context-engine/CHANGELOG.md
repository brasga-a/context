# Changelog

All notable changes to `context-engine` are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this crate follows
[Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Provisional version impact: **MINOR** (`0.2.0`) because the structural indexing and retrieval API
is being introduced.

### Added

- Added the initial `context-engine` library crate and crate documentation.
- Added derivation of nested, span-backed Markdown sections with deterministic heading paths and
  BLAKE3 content hashes.
- Added lenient YAML frontmatter interpretation with non-fatal diagnostics for invalid metadata.
- Added recursive Markdown vault indexing, body-free outlines, byte-exact section retrieval with
  provenance and suggestions, and deterministic fuzzy heading search.
- Added `VaultIndex::edit_section`: hash-guarded, body-only section replacement verified against
  fresh disk bytes, with section-escape rejection, a span-tight splice preserving all surrounding
  bytes, post-splice structural verification, and an atomic temp-file + rename write; returns the
  edited document's outline as `HashedOutlineSection` with fresh per-section hashes. Failures are
  typed via the new `EditError`.
- Added `VaultIndex::reindex_file` for refreshing one possibly stale index entry from disk.
- Added `Section::heading_span` recording the heading construct's own byte range (`None` for the
  preamble).
- Added a vault-wide wikilink graph: `[[Note]]`, `[[Note#Heading]]`, and `[[#Heading]]` targets
  are resolved (file part by filename stem or full path, heading part by heading text) into a
  `VaultIndex::backlinks(file, heading_path)` lookup returning `Backlink { from, raw_target,
  target_heading_path }`. Unresolved or ambiguous targets are non-fatal `VaultDiagnostic` entries.
  The link index is a whole-vault derived structure, rebuilt in full after every
  `edit_section`/`reindex_file` call so a change to one file's headings correctly flips
  resolution for links in other, untouched files.
- Added `VaultIndex::reindex_vault`: re-walks the vault root via `VaultIndex::build`, diffs the
  result against the index being replaced, swaps to it, and returns the diff as `VaultDiff`
  (`files_added`, `files_removed`, `files_changed: Vec<FileDiff>` with per-file
  `sections_added`/`sections_removed`/`sections_modified`). Fails without diffing or replacing
  the current index if the vault root is no longer valid. Reporting suppresses cascading and
  redundant entries: a modified heading path is omitted if a changed descendant exists, and an
  added/removed heading path is omitted if its parent was also added/removed — only the root
  cause of each change is reported. Renames (file or heading) are always reported as delete plus
  add, consistent with the existing heading-identity model; no rename detection is attempted.

### Changed

- `Provenance` (returned by `get_section` and `search`) now includes `content_hash`, the guard
  token for `edit_section`.
- `VaultIndex.diagnostics` now also carries wikilink-resolution diagnostics alongside the
  existing walk- and document-level ones.

## [0.1.0] - 2026-07-23

### Added

- Reserved the initial package version.
