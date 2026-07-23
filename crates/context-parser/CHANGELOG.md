# Changelog

All notable changes to `context-parser` are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this crate follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Provisional version impact: **MINOR** (`0.2.0`) because the public package and crate name changed during initial development.

### Added

- Added crate-level README and changelog documentation.

### Changed

- Adopted `context-parser` as the public package name and `context_parser` as the Rust crate name.

### Fixed

- Fixed link reference definition parsing to support transactional multiline labels, destinations, and titles with source-backed spans and exact paragraph-fragment consumption.
- Reconstructed contentless setext underlines after definition extraction as contiguous paragraph text or thematic breaks while preserving source spans in nested containers.

## [0.1.0] - 2026-07-19

### Added

- Introduced the source-backed block and inline parser, AST, diagnostics, definitions, and CommonMark, GFM, and Notes syntax support.
