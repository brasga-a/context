# context

Give AI agents the smallest exact slice of a Markdown knowledge base that answers their question —
and let them edit that slice back, safely.

`context` is an MCP server for Markdown vaults. It recursively indexes `.md` files when it starts,
exposes their heading structure and wikilink graph, and returns byte-exact sections with source
provenance. Agents can discover the right section before retrieving content instead of loading an
entire document into model context, then replace that section's body with a hash-guarded write
that leaves every other byte of the file untouched.

This makes retrieval more context-efficient by omitting unrelated body text. The actual token
savings depend on the document, selected section, MCP client, and model tokenizer; `context` does
not promise a fixed reduction.

## Quick start

From this repository, start the stdio MCP server with a vault directory:

```console
cargo run -- serve /absolute/path/to/vault
```

`context serve` is not an interactive prompt. It indexes the vault, then waits for an MCP client on
stdin and writes protocol responses to stdout. The client normally starts and owns this process.

To run a built or installed binary instead:

```console
context serve /absolute/path/to/vault
```

## Configure an MCP client

MCP clients wrap server settings differently, but a source-checkout configuration needs the same
command, arguments, working directory, and vault path:

```json
{
  "command": "cargo",
  "args": [
    "run",
    "--quiet",
    "--",
    "serve",
    "/absolute/path/to/vault"
  ],
  "cwd": "/absolute/path/to/context-repository"
}
```

For a built binary, use its absolute path as `command`, omit `cwd`, and pass
`["serve", "/absolute/path/to/vault"]` as `args`.

See the [complete MCP and CLI guide](docs/mcp-cli-usage.md) for lifecycle details, full request and
response examples, and troubleshooting.

## Agent retrieval workflow

Choose the shortest discovery path for what the agent already knows:

```text
topic only  -> search  -> choose result -> get_section
known file  -> outline -> choose path   -> get_section
```

If the agent knows only the topic, search headings and heading paths:

```json
{
  "query": "gun skill"
}
```

If it already knows the vault-relative file, inspect that document without loading its body:

```json
{
  "file": "player.md"
}
```

Both discovery tools return exact provenance. Copy the selected result's `file` and
`heading_path` directly into exact retrieval:

```json
{
  "file": "player.md",
  "heading_path": "Skills > Gun"
}
```

`get_section` returns only that section's byte-exact Markdown plus its file, heading path, byte
range, line range, and content hash — the token the next edit is guarded by.

## Editing a section

Every retrieval result carries a `content_hash`: the BLAKE3 hash of that section's exact source
bytes. To edit, pass it back as `expected_hash` along with the new body:

```json
{
  "file": "player.md",
  "heading_path": "Skills > Gun",
  "body": "Deal 3 damage per shot.",
  "expected_hash": "…the hash from get_section…"
}
```

`edit_section` re-reads the file from disk, checks the hash against its *current* bytes (not the
possibly-stale in-memory index), and only then splices in the new body — the heading line is
never touched. If the section changed since it was read, the call fails with a conflict naming
the current hash instead of silently overwriting. On success it returns the edited document's
outline with a fresh hash per section, so a follow-up edit needs no extra read.

An edit can only replace a section's *body*; the new content may not contain a heading at or
above the edited section's own level (that would restructure the document, which this tool
deliberately refuses to do).

## MCP tools

| Tool | Use it when | Input |
| --- | --- | --- |
| `search` | You know the topic, but not the file or exact heading | `query` |
| `outline` | You know the file and need its heading tree without body text | `file` |
| `get_section` | You have the exact result provenance and want source content | `file`, `heading_path` |
| `edit_section` | You want to replace a section's body, guarded by its content hash | `file`, `heading_path`, `body`, `expected_hash` |
| `backlinks` | You want every wikilink pointing at a file (or one section in it) | `file`, `heading_path` (optional) |

`search` is deterministic, case-insensitive matching over heading text and heading paths. It is
not semantic or embedding-based search. `outline` returns nested headings and line ranges without
section bodies. `get_section` requires the exact, case-sensitive breadcrumb returned by a
discovery tool.

## Wikilinks and backlinks

The vault's `[[target]]` wikilinks are resolved and indexed vault-wide, so `backlinks` can answer
"what links here" without an agent grepping raw file content itself:

- `[[Note]]` — a whole-file link, resolved by matching `Note` against every indexed file's
  filename stem.
- `[[Note#Heading]]` — resolved to a specific section, matched by heading text within `Note`.
- `[[#Heading]]` — a self-link, resolved within the file the link appears in.

A target that matches no file, or more than one, is unresolved rather than an error — the vault
still indexes, and the unresolved link shows up as a diagnostic. Resolution is exact, not fuzzy:
a typo in a link target does not get corrected or suggested.

## Vault behavior

- The vault root can contain nested directories; `.md` extensions are matched case-insensitively.
- Tool calls use vault-relative paths such as `lore/weapons.md`, never absolute source paths.
- The in-memory index is built once at startup.
- `edit_section` writes through to disk atomically and refreshes the in-memory index for the
  edited file in the same call — no restart needed for edits made through the tool.
- Changes made *outside* the server (another editor, `git checkout`, etc.) are not picked up
  automatically; restart the server to reindex them.
- The wikilink/backlink index is vault-wide and is fully recomputed after every `edit_section`
  call, since editing one file's headings can change whether a link in a different file resolves.

## Current boundaries

The current release does not include:

- file watching or automatic reindexing of changes made outside the server;
- a persistent index;
- semantic search or embeddings;
- section-body full-text search;
- snapshots or change-history tools;
- inserting, deleting, or renaming sections (only replacing a section's body); or
- rewriting wikilinks when the section they point to is renamed or removed.

## Workspace

- [`context-engine`](crates/context-engine/README.md) builds section trees, indexes vaults, and
  performs structural retrieval.
- [`context-lexer`](crates/context-lexer/README.md) provides the low-level streaming Markdown
  lexer.
- [`context-parser`](crates/context-parser/README.md) builds the source-backed Markdown document
  tree.

Run the standard development checks from the repository root:

```console
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
