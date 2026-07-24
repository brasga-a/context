## Context

The root `README.md` contains only `[[docs/mcp-cli-usage.md]]`, which is neither a portable
Markdown link nor a useful repository landing page. The detailed guide already documents startup,
configuration, tool schemas, troubleshooting, and boundaries accurately, while the implementation
and main specs define a small read-only MCP surface.

The README should serve AI users, agent builders, and evaluators who need to decide quickly whether
`context` fits their retrieval workflow. It must remain useful on common repository renderers,
avoid duplicating the entire detailed guide, and stay strictly within shipped behavior.

## Goals / Non-Goals

**Goals:**

- Make the value proposition understandable within the opening screen of the README.
- Give an AI-tool user the shortest accurate path from repository discovery to a configured MCP
  server.
- Explain how `search` or `outline` composes with `get_section` to reduce unnecessary model
  context while preserving exact source provenance.
- Present the three tools, recursive vault behavior, read-only guarantees, and startup-index
  lifecycle accurately.
- Route readers to the detailed MCP/CLI guide and crate-level contributor documentation.

**Non-Goals:**

- Changing the CLI, MCP tools, indexing, search ranking, or parser behavior.
- Adding client-specific setup for products whose configuration formats can change independently.
- Duplicating the full tool schemas, complete response payloads, or troubleshooting catalog from
  `docs/mcp-cli-usage.md`.
- Claiming semantic search, live file watching, writes, or a universal token-savings percentage.

## Decisions

### Use an AI-first information hierarchy

The README will lead with a plain-language product statement and a compact explanation of the
agent problem it solves. It will then present quick start, MCP configuration, the recommended
retrieval loop, a compact tool reference, current boundaries, and links for deeper reading.

This order is preferred over leading with workspace internals because the primary reader must
first decide whether and how to use the MCP server. Crate architecture remains discoverable near
the end for contributors.

### Keep the README executable but intentionally compact

Examples will use the real `cargo run -- serve <vault-dir>` command and a client-neutral stdio MCP
configuration. Tool examples will focus on the handoff of returned `file` and `heading_path` into
`get_section`, rather than reproducing the long payloads already maintained in the detailed guide.

This avoids a second full reference that could drift while still letting an AI-tool user connect
the server without inspecting source code.

### Describe context efficiency without a fixed benchmark

The README will explain that `outline` omits body text and `get_section` returns one exact section
instead of the whole document. It may describe this as token-efficient structural retrieval, but
will not promise a fixed percentage because savings depend on document shape, selected section,
MCP envelope, and model tokenizer.

This retains the core AI-user benefit without turning a one-file measurement into a product-wide
guarantee.

### Treat existing behavior and detailed documentation as sources of truth

Commands and tool names will be cross-checked against `src/main.rs`, `src/server.rs`, the main
OpenSpec capabilities, and `docs/mcp-cli-usage.md`. The README will use standard relative Markdown
links such as `docs/mcp-cli-usage.md`, not wiki-link syntax.

## Risks / Trade-offs

- [README duplicates operational facts that later change] → Keep examples minimal, link to the
  canonical detailed guide, and verify both documents against the same specs during implementation.
- [AI-first framing obscures the lexer/parser libraries] → Include a concise workspace section with
  links to each crate README rather than leading with library internals.
- [Token-efficient wording is interpreted as a guaranteed benchmark] → Explain the structural
  mechanism and avoid fixed savings claims.
- [Client-neutral configuration is less copy-paste-ready for one specific product] → Preserve the
  stable command/args/cwd contract and direct readers to adapt the surrounding client settings.

## Migration Plan

Replace the placeholder README in one documentation-only change, validate every command and link,
and review the rendered Markdown structure. No data, API, dependency, or runtime migration is
required. Reverting the README restores the prior state without affecting the executable.

## Open Questions

None. The current MCP surface and documentation hierarchy provide enough information to implement
the README without a product or runtime decision.
