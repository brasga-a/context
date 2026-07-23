## Why

The `context` binary now provides a working CLI and stdio MCP server, but its usage is only
summarized in developer-oriented repository context. Users need one practical guide that explains
how to build and launch the server, connect an MCP client, choose a vault, call each tool, and
diagnose common errors.

## What Changes

- Add `docs/mcp-cli-usage.md` as the canonical user guide for the `context` CLI and MCP server.
- Document prerequisites, build and invocation commands, vault indexing behavior, process
  lifecycle, and the distinction between vault-relative file paths and heading paths.
- Document MCP client configuration for a source checkout and a built binary without claiming
  client-specific behavior the project does not control.
- Provide request/response examples for `outline`, `get_section`, and `search`, including
  provenance fields, ranked search results, and helpful not-found errors.
- Add an end-to-end workflow and troubleshooting section covering invalid vaults, unknown files or
  heading paths, stdio transport expectations, and reindexing by restarting the process.
- Verify every command, tool name, argument, and example against the implemented CLI and MCP
  surface.

## Capabilities

### New Capabilities

- `mcp-cli-usage-guide`: A repository guide that accurately teaches users how to run, configure,
  and use the `context` CLI and its three MCP tools.

### Modified Capabilities

(none — this is a documentation-only change and does not alter runtime requirements)

## Impact

- Adds a new Markdown document under `docs/`.
- Does not change Rust source, public APIs, command syntax, MCP schemas, dependencies, or runtime
  behavior.
- Documentation examples must stay aligned with `src/main.rs`, `src/server.rs`, and the existing
  MCP integration fixture/tests.
