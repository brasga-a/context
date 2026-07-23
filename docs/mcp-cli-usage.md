# Using the `context` CLI and MCP server

`context` is a read-only context server for Markdown vaults. It indexes a directory of Markdown
files when it starts and exposes three MCP tools over stdio:

- `outline` returns a document's heading tree without its body text.
- `get_section` returns one byte-exact section with source provenance.
- `search` finds headings and heading paths from a rough query.

The current server keeps its index in memory. It does not modify the vault, and it serves until its
MCP client closes the stdio transport.

## Prerequisites

To run from a source checkout, you need:

- a Rust toolchain with edition 2024 support;
- Cargo on `PATH`; and
- a vault directory containing one or more `.md` files.

If you already have a built `context` executable, only the executable and vault are required.

## Quick start

From the repository root, launch the server through Cargo:

```console
cargo run -- serve /absolute/path/to/vault
```

Or build an optimized binary first:

```console
cargo build --release
```

Then launch that binary directly:

```console
./target/release/context serve /absolute/path/to/vault
```

If `context` is installed or otherwise available on `PATH`, the equivalent command is:

```console
context serve /absolute/path/to/vault
```

`context serve` is not an interactive shell command: after indexing, the process waits for
newline-delimited MCP messages on stdin and reserves stdout for MCP responses. Normally an MCP
client starts and owns this process.

## How the vault is indexed

`<vault-dir>` must exist and must be a directory. At startup, `context` recursively scans it and
indexes regular files whose extension is `.md` (case-insensitive). Other files are ignored.

MCP tool inputs identify files relative to this root, using paths such as:

```text
player.md
lore/weapons.md
```

Use vault-relative paths in tool calls, not absolute filesystem paths. Heading paths are separate
identifiers built from heading breadcrumbs, such as `Skills > Gun`.

The index is a startup snapshot. If a Markdown file is added, removed, renamed, or edited while the
server is running, restart the server so it rebuilds the in-memory index.

If the vault path is missing or is not a directory, startup exits non-zero before serving and names
the offending path. For example:

```text
context: vault path '/notes/missing' does not exist
```

## Configure an MCP client

MCP clients use different settings containers, but every stdio configuration needs the same core
launcher information:

- **command**: the executable to start;
- **arguments**: `serve` followed by the vault directory; and
- **working directory**: needed when launching through Cargo from a source checkout.

Prefer absolute executable, repository, and vault paths in persistent client configuration.

### Launch a built binary

Map this conceptual launcher specification into your client's MCP server settings:

```json
{
  "command": "/absolute/path/to/context",
  "args": ["serve", "/absolute/path/to/vault"]
}
```

If `context` is already on the client's `PATH`, `"command": "context"` is sufficient.

### Launch from a source checkout

To let Cargo build and run the server, configure the repository as the working directory:

```json
{
  "command": "cargo",
  "args": ["run", "--quiet", "--", "serve", "/absolute/path/to/vault"],
  "cwd": "/absolute/path/to/context-repository"
}
```

The `--` separates Cargo's options from the binary's `serve <vault-dir>` arguments. Your client may
name `cwd` differently or configure it outside the server entry; use the equivalent working-directory
setting it provides.

On initialization, the server identifies itself as `context` and advertises `get_section`,
`outline`, and `search`. Keep stdout connected exclusively to the MCP transport; compilation and
operational diagnostics belong on stderr.

## Tool reference

The examples below show the `arguments` sent in an MCP `tools/call` request and the corresponding
`structuredContent` payload. Your MCP client may render these as forms or native objects instead of
raw JSON.

### `outline`

Use `outline` when you know the file and need to discover its section structure without loading
body text.

Arguments:

```json
{
  "file": "player.md"
}
```

`file` is a vault-relative Markdown path. The result contains the same file identifier and a
nested `sections` array. Each section has:

- `heading`: plain heading text (`Preamble` for synthetic content before the first heading);
- `level`: the Markdown heading level (`0` for a synthetic preamble);
- `heading_path`: the exact, disambiguated breadcrumb accepted by `get_section`;
- `line_range`: one-based, inclusive start and end lines; and
- `children`: lower-level sections in document order.

For the fixture `player.md`, the structured result is:

```json
{
  "file": "player.md",
  "sections": [
    {
      "heading": "Skills",
      "level": 2,
      "heading_path": "Skills",
      "line_range": {"start": 4, "end": 11},
      "children": [
        {
          "heading": "Gun",
          "level": 3,
          "heading_path": "Skills > Gun",
          "line_range": {"start": 7, "end": 8},
          "children": []
        },
        {
          "heading": "Sword",
          "level": 3,
          "heading_path": "Skills > Sword",
          "line_range": {"start": 10, "end": 11},
          "children": []
        }
      ]
    },
    {
      "heading": "Inventory",
      "level": 2,
      "heading_path": "Inventory",
      "line_range": {"start": 13, "end": 14},
      "children": []
    }
  ]
}
```

Duplicate sibling headings receive stable suffixes such as `[2]`. Always copy the returned
`heading_path` rather than reconstructing it.

### `get_section`

Use `get_section` when you have both an indexed file and an exact heading path.

Arguments:

```json
{
  "file": "player.md",
  "heading_path": "Skills > Gun"
}
```

Both identifiers are exact. `file` is vault-relative; `heading_path` is the complete breadcrumb
returned by `outline` or `search`.

Structured result:

```json
{
  "content": "### Gun\nFire the equipped weapon.",
  "provenance": {
    "file": "player.md",
    "heading_path": "Skills > Gun",
    "byte_range": {"start": 58, "end": 91},
    "line_range": {"start": 7, "end": 8}
  }
}
```

`content` is the byte-exact source slice: no Markdown re-rendering or normalization is applied.
`byte_range` is half-open (`start` included, `end` excluded), while `line_range` is one-based and
inclusive. A parent section's slice includes its nested child sections.

If the path is not present, the call returns a caller-visible MCP tool error. Its structured content
names the missing path and supplies up to three nearest heading paths:

```json
{
  "message": "heading path 'Skills > Cannon' was not found in file 'player.md'",
  "suggestions": ["Skills > Gun", "Skills > Sword", "Skills"]
}
```

An unknown file similarly reports that the file is not indexed and suggests nearby indexed file
paths when available.

### `search`

Use `search` when you know the topic but not the file or exact heading path.

Arguments:

```json
{
  "query": "gun skill"
}
```

Search matches heading text and full heading paths, not section body text. Matching is
case-insensitive, tolerant of word order and partial tokens, and deterministic for an unchanged
vault. Higher `score` values rank first.

The result repeats the query and returns ranked matches with provenance. An abridged payload showing
the `player.md` match is:

```json
{
  "query": "gun skill",
  "results": [
    {
      "heading": "Gun",
      "score": 20100,
      "provenance": {
        "file": "player.md",
        "heading_path": "Skills > Gun",
        "byte_range": {"start": 58, "end": 91},
        "line_range": {"start": 7, "end": 8}
      }
    }
  ]
}
```

The full result array can contain additional matches before or after this entry. Treat `score` as a
ranking value; use the returned `file` and `heading_path`, rather than the score, to retrieve
content.

## Retrieval workflow

Choose the shortest discovery path for what you already know:

1. If you know the file, call `outline` with its vault-relative path.
2. If you only know the topic, call `search` with a few heading words.
3. Select the narrowest relevant match.
4. Pass its `file` and exact `heading_path` directly to `get_section`.
5. Use the returned source and provenance for the agent's task.

For example, an agent looking for the gun skill can call `search({"query": "gun skill"})`, select
the result with `file: "player.md"` and `heading_path: "Skills > Gun"`, then call:

```json
{
  "file": "player.md",
  "heading_path": "Skills > Gun"
}
```

This workflow retrieves only the relevant section instead of loading all of `player.md`.

## Troubleshooting

### The server exits immediately

Check stderr. The CLI accepts exactly:

```text
context serve <vault-dir>
```

Missing or extra arguments produce the usage error. A path that does not exist produces
`vault path '…' does not exist`; a file passed instead of a directory produces
`vault path '…' is not a directory`. Use an existing, readable directory—preferably an absolute
path in MCP client configuration.

### A file is reported as not indexed

- Confirm that the tool's `file` value is relative to the configured vault root.
- Use `/` between nested path components, for example `lore/weapons.md`.
- Confirm that the file has a `.md` extension and is readable.
- If the file was added or renamed after startup, restart the server.
- Check the tool error's `suggestions` array for nearby indexed paths.

### A heading path is not found

`get_section` requires an exact, complete, case-sensitive `heading_path`. Call `outline` for the
file or `search` for the topic, then copy the returned path verbatim. Pay attention to parent
breadcrumbs and duplicate suffixes such as `[2]`. The error's `suggestions` array lists the nearest
known paths in that file.

### Search returns no useful result

Search covers headings and heading paths only; it does not inspect body text. Try fewer heading
words or a partial token. If you know the file, use `outline` instead.

### MCP initialization or framing fails

- Ensure the client launches the command with `serve` and exactly one vault-directory argument.
- When using Cargo, set the working directory to the repository and retain the `--` argument
  separator.
- Do not place a wrapper in front of the server that prints banners or logs to stdout.
- Reserve stdout for newline-delimited MCP JSON-RPC messages; send diagnostics to stderr.
- Keep stdin and stdout open for the lifetime of the client session.

Running `context serve` directly in a terminal shows no interactive prompt. That is expected: the
process is waiting for an MCP initialization request on stdin.

### Results do not reflect a recent edit

Stop and restart the MCP server. The vault is parsed once at startup; there is no live file watcher
or `reindex` tool in the current version.

## Current boundaries

The current release intentionally provides structural, read-only retrieval. It does not provide:

- file watching or automatic reindexing;
- a persistent on-disk index;
- semantic or embedding-based search;
- section-body full-text search;
- snapshots, history, or `changes_since` diffs;
- vault write or mutation tools; or
- wikilink graph tools.

Restarting rebuilds the complete in-memory index. `search` uses deterministic normalized heading
and heading-path matching; it is not a semantic fallback.
