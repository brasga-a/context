## Context

The repo contains a two-crate parsing stack (`context-lexer` → `context-parser`) producing a
span-backed AST: every block and inline node records its absolute byte range in the source.
`Document.children` is a *flat* list — `Heading` is a sibling of the content that follows it, not
a container — and frontmatter is captured only as an uninterpreted span. The root `context` binary
is a "Hello, world!" placeholder whose purpose was left open in the archived
`add-claude-workflow` design; this change resolves it.

The product direction (decided in exploration): a local-first context engine for AI agents over
markdown vaults — Cursor-for-notes. The full vision includes LanceDB persistence, bundled-ONNX
embeddings, and content-addressed snapshots with `changes_since` diffs; this change deliberately
builds only the structural foundation those layers sit on.

## Goals / Non-Goals

**Goals:**
- A `context-engine` crate that turns a parsed document into a nested section tree with
  span-backed sections, heading-path breadcrumbs, and content hashes.
- Frontmatter parsed as YAML into document metadata.
- Structural retrieval: exact + fuzzy heading-path lookup, outlines, exact source slices.
- Vault indexing (a directory of `.md` files) held in memory.
- The `context` binary as an MCP server (stdio) exposing `outline`, `get_section`, `search`.

**Non-Goals:**
- No embeddings, no vector search (follow-up change; the two-path retrieval design means the
  structural path ships alone without degradation for exact/near-exact queries).
- No persistence — no LanceDB, no on-disk index; re-index on startup (vaults of markdown parse in
  milliseconds; persistence arrives with LanceDB).
- No snapshots, no `changes_since`, no file watching — index is built at startup; a manual
  `reindex` escape hatch is acceptable but live watching is out of scope.
- No writes — the engine is strictly read-only over the vault in this change.
- No wikilink graph tools yet (the parser exposes wikilinks; graph retrieval is a follow-up).

## Decisions

### 1. Sections are derived by the engine, not added to the parser
The parser stays CommonMark-faithful (flat blocks); `context-engine` folds the block stream into a
section tree: a section = one heading + following siblings until the next heading of
equal-or-higher level; lower-level headings nest as child sections. Content before the first
heading forms a synthetic "preamble" section. Section span = heading span start → end of last
contained block (nested sections' spans lie inside their parent's).
- **Alternative considered**: making the parser emit nested sections — rejected; it would couple
  CommonMark parsing to a retrieval-layer concept and break the parser's "source-backed, no
  interpretation" contract.

### 2. Section identity = heading path; hash = change detection
A section's identity is its breadcrumb path of heading texts (`Skills > Gun`), scoped to its file.
Its content hash (over the exact source bytes of its span) detects change. Renamed headings are
treated as delete + add — no rename heuristics. This was decided in exploration with the future
snapshot layer in mind: history will be content-addressed, so renames lose no data. Duplicate
sibling headings get a disambiguating index suffix (`Skills > Gun[2]`) so paths stay unique.

### 3. Fuzzy heading matching is deterministic string matching, not ML
`search("gun skill")` should match `## Gun Skill` via case-insensitive normalized token matching
(exact path match > exact heading match > substring/token-subset match, ranked). No embedding
model in this change — semantic fallback is the follow-up. Ties rank by path depth then file path
for determinism.

### 4. MCP server on stdio via the official Rust SDK (`rmcp`)
The `context` binary runs as an MCP stdio server — the transport every MCP client (Claude Code,
Cursor, etc.) supports without networking concerns. Vault root comes from a CLI argument
(`context serve <vault-dir>`). The binary keeps a thin CLI layer so future subcommands (`index`,
`snapshot`, …) have a home.
- **Alternative considered**: HTTP/SSE transport — deferred; stdio is the lowest-friction default
  and the SDK makes adding transports later cheap.

### 5. Tool responses return exact source slices plus provenance
`get_section` returns the literal bytes of the section's span (agents see the file exactly as
written) together with file path, heading path, byte range, and line range (via the parser's line
index). No markdown re-rendering, ever — re-rendering loses information and the spans make
fidelity free.

### 6. In-memory index, rebuilt on startup
The index is a per-file map of section trees plus a flat lookup table of (heading path → section)
across the vault. Parsing a typical vault is fast enough that persistence is premature here;
the LanceDB change owns durability. This keeps the foundation change free of storage-format
decisions that LanceDB would immediately supersede.

### 7. New dependencies, chosen minimal
- `rmcp` (official Rust MCP SDK) — server + stdio transport, in the binary only.
- `serde_yaml` (or `serde_yml` successor — verify maintenance status at implementation time) for
  frontmatter, in the engine crate.
- Content hash: `blake3` (fast, stable, already the de-facto choice for content addressing —
  matters because future snapshots key on these hashes).
- Fuzzy matching: hand-rolled token normalization first; pull in a crate only if ranking proves
  inadequate.

## Risks / Trade-offs

- [`rmcp` API is young and moves quickly] → Pin the version; keep MCP handling in one module of
  the binary so SDK churn stays contained.
- [Heading-path identity breaks on renames (known, accepted)] → Snapshot layer (follow-up) makes
  this lossless; until then a rename is just a re-served section under a new path.
- [Duplicate headings across a vault make `search` ambiguous] → Results always carry file + full
  path; ranking is deterministic; `get_section` requires the file argument so it is never
  ambiguous.
- [In-memory index means startup cost scales with vault size] → Acceptable for foundation; the
  parser is fast (3k-line crate parses its own test corpus in ms). Revisit with LanceDB change.
- [Frontmatter YAML in the wild is messy] → Parse leniently: on YAML error, record a diagnostic
  and treat metadata as absent — never fail indexing of a file over bad frontmatter.

## Migration Plan

Additive: new crate + binary growth; no existing crate's API changes. Implementation order:
(1) section tree + hashing in `context-engine` with unit tests against fixture documents,
(2) frontmatter metadata, (3) vault index + structural lookup/fuzzy search, (4) MCP server wiring
in the binary, (5) end-to-end verification driving the served tools against a fixture vault,
(6) `CLAUDE.md` update. Each step lands compiling and tested before the next.

## Open Questions

- Exact fuzzy-ranking details (token-subset scoring weights) — settle empirically during
  implementation against a fixture vault; the spec pins behavior only for exact and
  clear-substring cases.
- Does `search` also match against section *body* text (not just headings) in this change, or is
  body search strictly the embedding layer's job? Leaning: headings only for now — body search
  without ranking infrastructure produces noise.
- MCP resource exposure (files as MCP resources vs tools only) — tools only for now; resources can
  be added when a real client workflow demands them.
