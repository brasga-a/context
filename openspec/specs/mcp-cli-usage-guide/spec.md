## Purpose

Defines the user-facing usage guide for the current read-only `context` CLI and MCP server
workflow, including startup, client configuration, tool composition, and troubleshooting.

## Requirements

### Requirement: Canonical MCP and CLI guide
The repository SHALL contain `docs/mcp-cli-usage.md` as a user-facing guide for the `context` CLI
and MCP server. The guide SHALL identify its current read-only, startup-indexed scope and SHALL not
present deferred features as available.

#### Scenario: Reader identifies the supported surface
- **WHEN** a reader opens `docs/mcp-cli-usage.md`
- **THEN** the guide identifies `context serve <vault-dir>`, stdio transport, startup indexing,
  and the `outline`, `get_section`, and `search` tools as the supported surface

#### Scenario: Deferred behavior is not implied
- **WHEN** the guide discusses vault updates or retrieval capabilities
- **THEN** it states that file watching, persistence, semantic embeddings, snapshots, and write
  operations are not part of the current workflow

### Requirement: CLI startup and lifecycle instructions
The guide SHALL document prerequisites, source-checkout and direct-binary invocation forms, vault
directory expectations, successful process lifecycle, and fail-fast behavior for invalid or missing
vault directories.

#### Scenario: User starts from a source checkout
- **WHEN** a user follows the source-checkout quick start
- **THEN** the documented command uses `cargo run -- serve <vault-dir>` and explains that the
  process remains attached to stdio until the MCP client closes the transport

#### Scenario: User starts a built binary
- **WHEN** a user already has the `context` executable
- **THEN** the guide shows `context serve <vault-dir>` and explains that the directory is scanned
  recursively for Markdown files at startup

#### Scenario: User supplies an invalid vault
- **WHEN** the configured vault path is absent or is not a directory
- **THEN** the troubleshooting guidance explains that startup exits non-zero and names the
  offending path

### Requirement: MCP client configuration guidance
The guide SHALL show how an MCP client launches the server by supplying an executable command and
the `serve` plus vault-directory arguments. It SHALL distinguish source-checkout configuration from
direct-binary configuration and note that the surrounding settings format is client-specific.

#### Scenario: Client launches a direct binary
- **WHEN** a user maps the guide's direct-binary example into an MCP client
- **THEN** the client command points to `context` and its arguments contain `serve` followed by the
  vault directory

#### Scenario: Client launches through Cargo
- **WHEN** a user maps the source-checkout example into an MCP client
- **THEN** the command uses Cargo with the repository as its working directory and passes
  `-- serve <vault-dir>` to the binary

### Requirement: Accurate tool reference and composed workflow
The guide SHALL document the exact input fields and useful output fields for `outline`,
`get_section`, and `search`. It SHALL include a composed workflow in which a user discovers a
heading path and passes the returned file/path provenance directly to `get_section`.

#### Scenario: Known file is explored by outline
- **WHEN** a reader follows the `outline` example for `player.md`
- **THEN** the request uses a vault-relative `file` field and the described response includes
  heading text, level, full heading path, and line range without body content

#### Scenario: Exact section is retrieved
- **WHEN** a reader follows the `get_section` example for `player.md` and `Skills > Gun`
- **THEN** the request uses `file` and `heading_path`, and the described response includes the
  byte-exact Markdown plus file, heading path, byte range, and line range provenance

#### Scenario: Rough query is handed off to exact retrieval
- **WHEN** a reader follows the `search` example with `gun skill`
- **THEN** the guide explains ranked fuzzy heading matches and shows using a result's
  vault-relative file and full heading path as the next `get_section` inputs

### Requirement: Error and troubleshooting guidance
The guide SHALL explain the caller-visible behavior for unknown files and heading paths, including
nearest-match suggestions, and SHALL cover stdio and stale-index troubleshooting without advising
unsupported recovery actions.

#### Scenario: Heading path is wrong
- **WHEN** `get_section` cannot find the requested heading path
- **THEN** the guide explains that the tool error names the missing path and provides nearest
  existing heading-path suggestions

#### Scenario: Indexed content changed on disk
- **WHEN** a Markdown file changes after server startup
- **THEN** the guide instructs the user to restart the server to rebuild the in-memory index

#### Scenario: Stdio is polluted
- **WHEN** diagnosing MCP framing or initialization failures
- **THEN** the guide explains that stdout is reserved for MCP protocol messages and operational
  diagnostics belong on stderr

### Requirement: AI-oriented repository README
The repository SHALL provide a root `README.md` that introduces `context` to AI users and agent
builders as a read-only MCP server for token-efficient structural retrieval from Markdown vaults.
The introduction SHALL describe the shipped behavior without presenting deferred capabilities as
available.

#### Scenario: AI user understands the value proposition
- **WHEN** an AI user reads the opening portion of the root README
- **THEN** they can identify that `context` indexes Markdown, exposes retrieval tools over MCP, and
  can return a relevant section instead of requiring a whole document in model context

#### Scenario: Reader is not promised unsupported behavior
- **WHEN** the README summarizes the current product surface
- **THEN** it identifies the read-only startup-indexed model and does not imply semantic search,
  automatic reindexing, persistence, or write operations

### Requirement: Minimal MCP onboarding from the README
The root README SHALL provide an accurate minimal startup command and client-neutral MCP launcher
configuration using `serve` plus a vault directory. It SHALL direct readers to
`docs/mcp-cli-usage.md` for the complete CLI, configuration, tool, and troubleshooting reference.

#### Scenario: User launches from the source checkout
- **WHEN** a reader follows the README quick start
- **THEN** the command uses `cargo run -- serve <vault-dir>` from the repository root

#### Scenario: User configures an MCP client
- **WHEN** a reader adapts the README launcher configuration
- **THEN** command, arguments, working-directory needs, and the vault path are distinguishable
  without relying on a product-specific settings wrapper

#### Scenario: User needs detailed guidance
- **WHEN** a reader needs full tool schemas, response examples, lifecycle details, or
  troubleshooting
- **THEN** a standard relative Markdown link opens `docs/mcp-cli-usage.md`

### Requirement: Agent retrieval workflow in the README
The root README SHALL explain the recommended agent workflow and the distinct purpose of
`search`, `outline`, and `get_section`. The workflow SHALL pass vault-relative `file` and exact
`heading_path` provenance from discovery directly into exact retrieval.

#### Scenario: Agent knows only the topic
- **WHEN** an agent knows a topic but not its file or heading path
- **THEN** the README directs it to call `search`, select a narrow result, and pass the returned
  `file` and `heading_path` to `get_section`

#### Scenario: Agent knows the file
- **WHEN** an agent knows the vault-relative file but not its section path
- **THEN** the README directs it to call `outline` and copy the returned exact `heading_path` into
  `get_section`

#### Scenario: Context-efficiency claim remains qualified
- **WHEN** the README explains why section retrieval is useful for AI context
- **THEN** it attributes savings to omitting unrelated document content and does not claim a fixed
  token-reduction percentage

### Requirement: Repository navigation for users and contributors
The root README SHALL use portable Markdown links to route readers to the detailed MCP/CLI guide
and the lexer, parser, and engine crate documentation.

#### Scenario: Links render outside a wiki-aware editor
- **WHEN** the README is rendered by a standard Markdown repository viewer
- **THEN** its documentation links resolve without requiring wiki-link syntax
