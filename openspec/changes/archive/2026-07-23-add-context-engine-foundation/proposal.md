## Why

This project is pivoting from its notepad origins into a **context engine for AI agents** — the
markdown equivalent of what Cursor does for codebases. An agent told "read `player.md` and
implement the gun skill" should receive only the `Skills > Gun` section (~40 lines), not the whole
file, keeping its context window small and focused. The hard substrate already exists: a
CommonMark/GFM/Notes parser (`context-parser`) whose every node carries absolute source spans.
What's missing is everything above it: sections don't exist as a concept (headings are flat
siblings of content in the AST), there is no index of any kind, and no way for an agent to talk to
the engine. This change builds that structural foundation — the retrieval path that answers the
most common class of agent queries exactly and cheaply, before any embeddings or vector search
enter the picture.

## What Changes

- New crate `context-engine`: derives a **section tree** from the parser's flat block stream (a
  section = heading + all following blocks until the next heading of equal-or-higher level, nested
  by level), with each section carrying its absolute source span, heading-path breadcrumb (e.g.
  `Skills > Gun`), and a content hash of its source bytes (groundwork for future delta detection).
- Frontmatter interpretation: parse the frontmatter span captured by `context-parser` as YAML and
  expose it as document metadata.
- **Structural retrieval**: exact and fuzzy heading-path lookup (`"gun skill"` matches
  `## Gun Skill`) resolving to exact source slices, plus per-document outlines.
- The placeholder `context` binary (`src/main.rs`) becomes real: an **MCP server** (stdio
  transport) exposing the engine to any AI agent, with an initial tool surface:
  - `outline(file)` — the heading tree of a document
  - `get_section(file, heading_path)` — exact source slice of one section
  - `search(query)` — fuzzy heading-path match across the indexed vault
- Vault indexing: point the engine at a directory of markdown files; it parses and indexes all of
  them (in-memory for this change; persistence comes with the LanceDB change).
- Resolves the `src/main.rs` open question from the archived `add-claude-workflow` design: the
  binary is the MCP server / CLI front-end.

**Deferred to follow-up changes** (decided during exploration, recorded in design.md): LanceDB
persistence and online-sync mode; bundled-ONNX embeddings for semantic search; content-addressed
snapshots and the `changes_since` diff tool.

## Capabilities

### New Capabilities
- `section-tree`: Deriving nested, span-backed, hashed sections (with heading-path identity) from
  a parsed markdown document, including frontmatter metadata.
- `structural-retrieval`: Exact and fuzzy heading-path lookup, document outlines, and exact
  source-slice extraction across an indexed vault of markdown files.
- `mcp-server`: The `context` binary serving the engine to AI agents over MCP (stdio), with
  `outline`, `get_section`, and `search` tools.

### Modified Capabilities
(none — `claude-dev-workflow` is unaffected; no requirement changes to existing specs)

## Impact

- New crate: `crates/context-engine` (depends on `context-parser`; new deps: a YAML parser for
  frontmatter, a fuzzy-matching approach for heading lookup).
- `src/main.rs` / root `Cargo.toml`: the placeholder binary gains real dependencies (MCP SDK,
  `context-engine`) and becomes the MCP server entry point.
- Root `Cargo.toml` workspace `members` gains the new crate.
- `CLAUDE.md`: updated to describe the new crate and the binary's role.
- No changes to `context-lexer` or `context-parser` public APIs (the engine consumes them as-is).
