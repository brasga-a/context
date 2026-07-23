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

## [0.1.0] - 2026-07-23

### Added

- Reserved the initial package version.
