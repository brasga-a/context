# Changelog

All notable changes to the `context` binary are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this package
follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Provisional version impact: **MINOR** (`0.2.0`) because the placeholder binary now exposes a new
MCP server CLI.

### Added

- Added `context serve <vault-dir>`, a stdio MCP server advertising `outline`, `get_section`, and
  `search` over an in-memory Markdown vault index.
- Added fail-fast vault validation and end-to-end MCP protocol coverage.
- Added the `edit_section` tool: hash-guarded, body-only section replacement returning the edited
  document's outline with fresh per-section hashes; conflict errors carry the section's current
  hash. End-to-end stdio coverage for the read → edit → conflict → escape-rejection loop.
- Added the `backlinks` tool: every indexed wikilink resolving to a file, optionally narrowed to
  one heading path, with full section provenance per link. End-to-end stdio coverage for
  resolved whole-file and file+heading links, narrowing by heading path, and unresolved links
  coexisting safely with normal tool operation.
- Added the `reindex_vault` tool (no parameters): re-walks the vault root and returns what
  changed since the previous index — the first tool that lets an agent notice changes made
  outside the server. End-to-end stdio coverage for an external file add, an external section
  edit, a no-op reindex, and a missing-root failure that leaves the previous index in place.

### Changed

- Resolved the root binary's placeholder role as the MCP front-end for `context-engine` — now
  read-write at section granularity, no longer read-only.
- The vault index sits behind a read-write lock: read tools share it, `edit_section` and
  `reindex_vault` serialize writes, and `get_section` / `search` responses now include each
  section's `content_hash`.
