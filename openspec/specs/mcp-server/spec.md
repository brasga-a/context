## Purpose

Defines the stdio MCP server exposed by the `context` binary and its structural-retrieval tools.

## Requirements

### Requirement: MCP server over stdio
The `context` binary SHALL run as an MCP server on stdio when invoked with a vault directory
(`context serve <vault-dir>`), indexing the vault at startup and serving MCP clients until the
transport closes. An invalid or missing vault directory SHALL fail startup with a clear error.

#### Scenario: Server starts and completes MCP initialization
- **WHEN** `context serve <valid-vault-dir>` is launched and an MCP client performs the
  initialization handshake over stdio
- **THEN** the handshake succeeds and the server advertises its tools

#### Scenario: Invalid vault directory fails fast
- **WHEN** `context serve` is pointed at a path that does not exist or is not a directory
- **THEN** the process exits non-zero with an error naming the offending path, before serving

### Requirement: outline tool
The server SHALL expose an `outline` tool taking a vault-relative file path and returning the
document's outline (per the structural-retrieval capability).

#### Scenario: Agent requests a file outline
- **WHEN** a client calls `outline` with `player.md`
- **THEN** the tool returns the section tree of `player.md` as heading text, level, heading path,
  and line range per section

### Requirement: get_section tool
The server SHALL expose a `get_section` tool taking a vault-relative file path and a heading path,
returning the exact source slice of that section with provenance (file, heading path, byte range,
line range). Errors from retrieval (unknown file/path) SHALL surface as MCP tool errors carrying
the underlying not-found message and suggestions.

#### Scenario: Agent retrieves one section instead of the whole file
- **WHEN** a client calls `get_section` with `player.md` and `Skills > Gun`
- **THEN** the tool returns the byte-exact source of only that section, with provenance

#### Scenario: Helpful error for a wrong path
- **WHEN** a client calls `get_section` with a heading path that does not exist
- **THEN** the tool returns an error containing the missing path and the nearest existing heading
  paths

### Requirement: search tool
The server SHALL expose a `search` tool taking a free-text query and returning ranked fuzzy
heading matches across the vault (per the structural-retrieval capability), each with enough
provenance to call `get_section` directly.

#### Scenario: Agent locates a section by rough description
- **WHEN** a client calls `search` with `gun skill`
- **THEN** results include the `Skills > Gun` section of `player.md` with file path and heading
  path, and the client can pass those directly to `get_section`
