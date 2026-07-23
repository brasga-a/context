## Context

The repository has a functional `context serve <vault-dir>` CLI and an MCP stdio server with
`outline`, `get_section`, and `search`, but no `docs/` content. `CLAUDE.md` gives contributors a
short startup summary; it is not a user guide and does not explain client configuration, tool
payloads, result fields, workflows, or failure modes. This change is documentation-only and must
describe the behavior already established by the binary, server implementation, engine API, and
end-to-end tests.

## Goals / Non-Goals

**Goals:**

- Create one canonical, task-oriented guide at `docs/mcp-cli-usage.md`.
- Give a new user enough information to build or locate the binary, choose a Markdown vault,
  launch the stdio server, configure an MCP client, and use every advertised tool.
- Explain exact path semantics, provenance, fuzzy result handoff, startup indexing, and useful
  troubleshooting in language that does not require knowledge of the Rust implementation.
- Keep commands and examples directly traceable to current source and integration tests.

**Non-Goals:**

- No changes to CLI parsing, MCP schemas, engine behavior, dependencies, or release packaging.
- No client-specific installation wizard or exhaustive configuration catalog.
- No documentation for deferred persistence, file watching, embeddings, snapshots, or write
  operations.
- No automated documentation generator or new documentation toolchain.

## Decisions

### 1. Use one progressive, workflow-oriented guide

The guide will progress from prerequisites and startup, through MCP connection, into tool reference,
an end-to-end retrieval workflow, and troubleshooting. Readers can stop after the quick start or
continue into exact payload and response details.

- **Alternative considered:** separate CLI, MCP configuration, and tool-reference pages.
- **Why rejected:** the initial surface is small, and splitting it would force readers to navigate
  several short pages to complete one setup workflow.

### 2. Treat code and protocol tests as the source of truth

CLI syntax will be checked against `src/main.rs`; tool names, arguments, and response shapes against
`src/server.rs` and `context-engine` result types; lifecycle and error examples against
`tests/mcp_stdio.rs`. The guide will not document planned commands or tools.

- **Alternative considered:** derive the guide only from the OpenSpec artifacts.
- **Why rejected:** planning artifacts express requirements, while executable source and tests
  capture the exact shipped spelling and JSON shape users must follow.

### 3. Show both source-checkout and built-binary launch forms

The quick start will show `cargo run -- serve <vault-dir>` for contributors and a direct
`context serve <vault-dir>` form for a built or installed executable. MCP configuration will use a
client-neutral command/arguments example and explicitly note that configuration containers differ
between clients.

- **Alternative considered:** document one named MCP client.
- **Why rejected:** it would date the guide around a third-party UI and imply client behavior this
  repository does not own.

### 4. Teach tool composition, not isolated calls only

The guide will recommend `outline(file)` when the file is known, `search(query)` when the heading is
uncertain, and then `get_section(file, heading_path)` using provenance returned by the first call.
Examples will use the established `player.md` / `Skills > Gun` vocabulary and clearly distinguish
vault-relative file paths from exact heading-path breadcrumbs.

- **Alternative considered:** provide a flat parameter table without a workflow.
- **Why rejected:** parameters alone do not explain how an agent discovers the exact
  `heading_path` required by `get_section`.

### 5. Keep verification lightweight and repository-native

Implementation will review all snippets against current source and run the existing MCP integration
test plus the standard formatting/check workflow. No documentation framework or generated output is
introduced for one Markdown page.

- **Alternative considered:** add a documentation site or snippet-testing dependency.
- **Why rejected:** the maintenance cost is disproportionate to the single-page scope.

## Risks / Trade-offs

- [Client configuration formats vary] → Label the configuration as a command/argument model and
  instruct readers to map it into their client's MCP server settings.
- [Examples can drift from code] → Cite the relevant command/tool names exactly and make
  source/test comparison an explicit implementation task.
- [A comprehensive page can become hard to scan] → Lead with a minimal quick start, use clear
  headings and tables, and move detailed response/error material after the core workflow.
- [Users may assume live file watching] → State prominently that the vault is indexed at startup
  and changes require restarting the server.

## Migration Plan

Add the new Markdown page under `docs/`, verify its examples, and ship it with the repository. There
is no runtime migration or rollback requirement; rollback is removal of the documentation file.

## Open Questions

(none — the current CLI and MCP surface are sufficiently defined for the guide)
