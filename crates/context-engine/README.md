# context-engine

`context-engine` builds span-backed section trees and structural indexes over Markdown documents
parsed by `context-parser`. It supports document metadata, exact section retrieval, outlines,
deterministic fuzzy heading search, hash-guarded section editing (`VaultIndex::edit_section`), a
vault-wide wikilink graph (`VaultIndex::backlinks`), and vault-wide reindexing with a change
report (`VaultIndex::reindex_vault`).
