## 1. AI-first README foundation

- [x] 1.1 Replace the placeholder root `README.md` with a clear title, concise product
      description, and opening explanation of the problem `context` solves for AI agents
- [x] 1.2 Explain recursive Markdown-vault indexing, exact section provenance, and the
      context-efficiency benefit of retrieving a narrow section instead of unrelated document body
      content, without promising a fixed token-reduction percentage

## 2. MCP onboarding and agent workflow

- [x] 2.1 Add a minimal source-checkout quick start using `cargo run -- serve <vault-dir>` and a
      client-neutral stdio MCP launcher example with distinguishable command, arguments, working
      directory, and vault path
- [x] 2.2 Add a compact reference for `search`, `outline`, and `get_section` that accurately
      distinguishes discovery from exact retrieval
- [x] 2.3 Document the two recommended agent paths—topic → `search` → `get_section` and known file
      → `outline` → `get_section`—including the direct handoff of vault-relative `file` and exact
      `heading_path` provenance

## 3. Scope and repository navigation

- [x] 3.1 State the current read-only, startup-indexed boundaries and distinguish deterministic
      heading/path search from semantic search, file watching, persistence, and write operations
- [x] 3.2 Add portable relative Markdown links to `docs/mcp-cli-usage.md` and the README files for
      `context-engine`, `context-lexer`, and `context-parser`

## 4. Verification

- [x] 4.1 Cross-check every README command, tool name, input field, lifecycle statement, and
      capability claim against `src/main.rs`, `src/server.rs`, the main OpenSpec capabilities, and
      `docs/mcp-cli-usage.md`
- [x] 4.2 Validate fenced JSON examples, verify every repository-relative link resolves, and review
      the rendered heading order for a concise AI-first reading path
- [x] 4.3 Run the repository `/check` workflow and review the final diff to confirm the
      implementation is documentation-only and introduces no unrelated changes
