## 1. Guide foundation and startup

- [x] 1.1 Create `docs/mcp-cli-usage.md` with a concise overview, supported/read-only scope,
      prerequisites, and a quick-start path for both source checkouts and built binaries
- [x] 1.2 Document vault selection, recursive `.md` indexing at startup, stdio process lifecycle,
      invalid-vault failure behavior, and the need to restart after on-disk changes
- [x] 1.3 Add client-neutral MCP launch configuration examples for a direct `context` binary and
      for `cargo run`, clearly separating command, arguments, working directory, and vault path

## 2. MCP tools and retrieval workflow

- [x] 2.1 Document `outline`: exact `file` input semantics, body-free nested output, heading paths,
      and line ranges, with a `player.md` example
- [x] 2.2 Document `get_section`: exact `file` + `heading_path` inputs, byte-exact content,
      provenance fields, and nearest-heading error behavior, using `Skills > Gun`
- [x] 2.3 Document `search`: free-text query behavior, ranked result provenance, and a `gun skill`
      example that hands the returned file/path directly to `get_section`
- [x] 2.4 Add an end-to-end decision workflow explaining when to use `outline` versus `search` and
      how both lead to exact retrieval without loading a whole document

## 3. Troubleshooting and boundaries

- [x] 3.1 Add troubleshooting for missing/invalid vaults, unknown files, wrong heading paths, MCP
      initialization/framing issues, stdout protocol purity, and stale startup indexes
- [x] 3.2 State current non-capabilities explicitly: no file watching, persistent index, semantic
      embeddings, snapshots/diffs, or write operations

## 4. Verification

- [x] 4.1 Cross-check every command, tool name, input field, response field, and error claim against
      `src/main.rs`, `src/server.rs`, `context-engine` result types, and `tests/mcp_stdio.rs`
- [x] 4.2 Run `cargo test --test mcp_stdio` to verify the documented end-to-end protocol examples
      remain aligned with the executable server
- [x] 4.3 Run the repository `/check` workflow and review the final diff to confirm the
      implementation is documentation-only, well-structured, and free of broken repository paths
