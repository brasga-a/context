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

### Changed

- Resolved the root binary's placeholder role as the read-only MCP front-end for
  `context-engine`.
