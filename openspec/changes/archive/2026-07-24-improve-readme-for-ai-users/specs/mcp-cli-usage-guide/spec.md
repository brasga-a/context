## ADDED Requirements

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
