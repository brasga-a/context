## Why

The root `README.md` is currently only a wiki-style link, so AI users and agent builders cannot
quickly understand what `context` does, why structural retrieval is useful, or how to connect it to
an MCP client. The repository needs an AI-first landing page that communicates the working
read-only workflow accurately and directs readers to deeper operational documentation.

## What Changes

- Replace the placeholder root README with a concise explanation of `context` as a token-efficient
  MCP server for Markdown vaults.
- Lead with the agent retrieval workflow: discover with `search` or `outline`, then retrieve the
  narrowest exact section with `get_section`.
- Add a minimal quick start and client-neutral MCP configuration that an AI tool user can adapt
  without reading implementation source.
- Explain recursive vault indexing, vault-relative provenance, and why section-level retrieval
  avoids loading whole documents into model context.
- State the current read-only and startup-indexed boundaries without presenting deferred features
  as available.
- Link to the detailed MCP/CLI guide and contributor-oriented crate documentation using portable
  Markdown links.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mcp-cli-usage-guide`: Extend the user-facing documentation contract with an AI-oriented root
  README that provides accurate product positioning, a minimal MCP onboarding path, an
  agent-friendly retrieval workflow, and links to canonical detailed documentation.

## Impact

- Updates the repository-root `README.md` and the existing `mcp-cli-usage-guide` specification.
- Does not change the CLI, MCP protocol, Rust crates, dependencies, vault format, or runtime
  behavior.
