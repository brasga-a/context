## Context

`context-engine` derives span-backed section trees: each `Section` records the absolute half-open
byte range of heading + body and a BLAKE3 hash of exactly those bytes
(`crates/context-engine/src/section.rs`). `VaultIndex` holds one `EngineDocument` (full source +
tree) per vault-relative file path, built once at startup; every method takes `&self` and the MCP
server (`src/server.rs`) serves `outline` / `get_section` / `search` from that immutable index.
The foundation design explicitly deferred writes ("the engine is strictly read-only over the vault
in this change"); this change is that follow-up, shaped in the 2026-07-24 exploration session.

## Goals / Non-Goals

**Goals:**
- One write primitive: replace a section's body, heading preserved, addressed by
  file + heading path.
- Optimistic concurrency via the existing `content_hash`, verified against fresh disk bytes.
- Reject edits that would restructure the document (section escape).
- Deterministic boundary normalization so a valid edit always yields a well-formed document.
- Atomic on-disk write; index updated by re-parsing the one edited document.
- `edit_section` MCP tool returning the fresh outline with new hashes.

**Non-Goals:**
- No insert / delete / rename section ops (follow-up; rename may bring wikilink fix-up).
- No heading edits of any kind through this tool — the heading line is engine-copied, never
  agent-supplied.
- No multi-section or multi-file transactions.
- No file watching — the read path may still serve a stale index after external edits; only the
  write path re-reads disk. Watching remains its own future change.
- No frontmatter editing (the preamble/frontmatter region is not addressable by this tool).

## Decisions

### 1. Disk, not index, is the write-time source of truth
The edit re-reads the target file from disk, re-derives its section tree, locates the heading
path, and compares the found section's hash against `expected_hash`. Only then does it splice.
This makes writes correct even when the in-memory index is stale (external editor touched the
file), at the cost of one extra parse per edit — milliseconds, per the foundation's measurements.
- **Alternative considered**: validate against the in-memory index — rejected; a stale index
  would let a "matching" hash authorize a splice at byte offsets that no longer correspond to the
  section, silently corrupting the file. The index is a cache; disk is truth.

### 2. Body-only replacement; the heading line is never agent-supplied
The tool replaces the byte range from the end of the heading line to the section span's end
(including all nested child sections — the body is everything the section contains). The heading
line itself is copied from the current file.
- Keeps `heading_path` valid across the edit, so an agent can re-address the same section
  immediately.
- Makes renames impossible by construction rather than by validation; rename becomes a deliberate
  future op that can also fix inbound wikilinks.
- The hash guard stays the **whole-section** hash (heading + body) exactly as stored on
  `Section` — it detects any drift, including an out-of-band heading rename, and requires no new
  hash variant.

### 3. Section escape is rejected, not absorbed
The new body is parsed standalone; if it contains any heading of level ≤ the target section's
level, the edit fails with an error naming the offending heading and level. An ATX `# H1` inside
a level-3 section would otherwise terminate the section early and re-parent everything after it —
a whole-document restructure smuggled through a body edit. Deeper headings (level > target) are
fine: they are legitimate child sections.
- **Alternative considered**: allow and return the new outline — rejected for v1; agents get a
  predictable invariant (an edit changes exactly one section's content, never the document's
  shape). Restructuring deserves its own explicit op.
- The check runs on the **top-level blocks** of the parsed body (parsed with a leading newline
  guard so a body starting with `---` cannot masquerade as frontmatter and smuggle headings past
  the check). Setext headings are ordinary heading blocks and are caught. Headings nested inside
  the body's own container blocks (e.g. inside a blockquote) are *allowed* — the section tree
  derives only from top-level headings, so they cannot restructure anything.
- Belt and braces: after the splice (before writing), the whole new document is re-parsed and its
  section skeleton (heading path + level of every section outside the edited one) is compared to
  the pre-edit skeleton. Any difference — e.g. a rare setext-underline glue across the section
  boundary — rejects the edit with a restructure error and no write. This makes silent corruption
  structurally impossible rather than merely unlikely.

### 4. Span-tight splice; separators preserved verbatim
Section spans end at their last content byte — the newline separators between sections live
*outside* every span. The splice honors that: it replaces exactly the byte range from the end of
the heading line to `span.end` with the new body, trimmed of trailing whitespace-only lines so it
too ends at a content byte. Every byte outside that range — including the original inter-section
separator, whatever it was — is preserved byte-identically, which makes the "untouched
surroundings" guarantee trivial instead of a normalization promise. Interior body bytes are
written verbatim (code blocks, trailing spaces, whatever the agent sent). An empty body (after
trimming) is legal and leaves just the heading line, with the heading's own line terminator
handling folded into the splice. The splice is one function, unit-tested — this is where bugs
would live. Structural safety across the preserved separator (e.g. a new trailing paragraph
gluing into a following setext underline) is guaranteed by the post-splice verification of
Decision 3, not by rewriting separators.

### 5. Atomicity and index update
The new file content is written to a temp file in the same directory and renamed over the
original (same-volume rename; the vault root is one volume by construction). On success the
engine re-parses that one document via a new `VaultIndex::reindex_file(&mut self, file)` and the
tool builds its response from the fresh tree. On any failure before rename, the original file is
untouched and the index is not modified.
- **Windows note (resolved)**: `std::fs::rename` on Windows corresponds to `MoveFileExW` with
  `MOVEFILE_REPLACE_EXISTING` (per std docs), so rename-over-existing replaces atomically; the
  e2e test exercises this on the Windows dev platform.
- Supporting change: `Section` gains a `heading_span: Option<Span>` field (`None` for the
  preamble) recorded during tree derivation, so the edit path knows where the heading construct
  ends (ATX line or setext underline) without re-walking parser blocks.

### 6. Concurrency: `RwLock<VaultIndex>` in the server, `&mut` in the engine
The engine stays lock-free: reads `&self`, the new reindex `&mut self`. The server wraps the
index in `std::sync::RwLock`; read tools take read guards, `edit_section` holds the write guard
across verify → splice → rename → reindex, serializing writes and preventing a read of the
half-updated index. Lock scope is one tool call — no async I/O held across await points beyond
what `rmcp`'s handler model already implies (file I/O is synchronous and fast).

### 7. Hashes flow through both ends of the loop
Outbound: `edit_section` returns the edited document's outline via a dedicated
`HashedOutlineSection` type where each entry carries its new `content_hash` (the read-path
`OutlineSection` stays hash-free and shape-stable). Without this the agent's very next edit to
the same file is guaranteed a conflict and a wasted round-trip. Inbound: the agent must hold a
hash *before* its first edit, and no read tool exposed one — so `Provenance` (returned by
`get_section` and every `search` result) gains a `content_hash` field. Additive, one struct, and
it makes read → guarded edit a two-call loop with no outline detour.

## Risks / Trade-offs

- **Read-path staleness unchanged** — reads can still serve pre-edit content if an external
  editor changed the file. Mitigated for writes (Decision 1); accepted for reads until the
  watcher change.
- **Whole-section hash means child edits conflict with parent edits** — editing a child section
  changes the parent's hash too (spans nest). Correct behavior (the parent's content *did*
  change), but agents holding old parent hashes must re-read; returning fresh hashes (Decision 7)
  keeps that cheap.
- **Normalization rewrites edge whitespace** — an agent intentionally sending trailing blank
  lines loses them. Acceptable: markdown semantics are unchanged, and determinism wins.
- **Temp-file strategy on Windows** needs the replace-semantics verification flagged in
  Decision 5; e2e test must run on Windows CI (the dev platform).

## Migration

No data or API migration. Existing read tools and their responses are byte-identical. New engine
API is additive (MINOR). Server internals change (lock wrapping) with no protocol-visible effect
on existing tools.

## Open Questions

(none — v1 scope was deliberately cut in exploration; deferred ops are listed in the proposal)
