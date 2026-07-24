# context

Give AI agents the smallest exact slice of a Markdown knowledge base that answers their question.

`context` is a read-only MCP server for Markdown vaults. It recursively indexes `.md` files when
it starts, exposes their heading structure, and returns byte-exact sections with source
provenance. Agents can discover the right section before retrieving content instead of loading an
entire document into model context.

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
range, and line range.

## MCP tools

| Tool | Use it when | Input |
| --- | --- | --- |
| `search` | You know the topic, but not the file or exact heading | `query` |
| `outline` | You know the file and need its heading tree without body text | `file` |
| `get_section` | You have the exact result provenance and want source content | `file`, `heading_path` |

`search` is deterministic, case-insensitive matching over heading text and heading paths. It is
not semantic or embedding-based search. `outline` returns nested headings and line ranges without
section bodies. `get_section` requires the exact, case-sensitive breadcrumb returned by a
discovery tool.

## Vault behavior

- The vault root can contain nested directories; `.md` extensions are matched case-insensitively.
- Tool calls use vault-relative paths such as `lore/weapons.md`, never absolute source paths.
- The in-memory index is built once at startup.
- Additions, edits, renames, and deletions require restarting the server.
- The server never modifies vault files.

## Current boundaries

The current release provides structural, read-only retrieval. It does not include:

- file watching or automatic reindexing;
- a persistent index;
- semantic search or embeddings;
- section-body full-text search;
- snapshots or change-history tools; or
- write operations.

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
