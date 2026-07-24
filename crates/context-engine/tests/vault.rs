use std::path::PathBuf;

use context_engine::VaultIndex;

fn fixture_vault() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault")
}

#[test]
fn indexes_nested_markdown_and_ignores_other_files() {
    let index = VaultIndex::build(fixture_vault()).expect("fixture vault indexes");

    assert_eq!(
        index.files().collect::<Vec<_>>(),
        ["lore/weapons.md", "player.md"]
    );
    assert!(index.diagnostics.is_empty());
}

#[test]
fn outline_preserves_tree_shape_paths_and_line_ranges_without_body_text() {
    let index = VaultIndex::build(fixture_vault()).expect("fixture vault indexes");
    let outline = index.outline("player.md").expect("player outline");

    assert_eq!(outline.len(), 2);
    assert_eq!(outline[0].heading, "Skills");
    assert_eq!(outline[0].level, 2);
    assert_eq!(outline[0].heading_path, "Skills");
    assert_eq!(outline[0].line_range.start, 4);
    assert_eq!(outline[0].line_range.end, 11);
    assert_eq!(outline[0].children.len(), 2);
    assert_eq!(outline[0].children[0].heading_path, "Skills > Gun");
    assert_eq!(outline[0].children[0].line_range.start, 7);
    assert_eq!(outline[0].children[0].line_range.end, 8);
    assert_eq!(outline[1].heading_path, "Inventory");
}

#[test]
fn exact_retrieval_returns_source_and_helpful_errors() {
    let index = VaultIndex::build(fixture_vault()).expect("fixture vault indexes");
    let section = index
        .get_section("player.md", "Skills > Gun")
        .expect("known section");

    assert_eq!(section.content, "### Gun\nFire the equipped weapon.");
    assert_eq!(section.provenance.file, "player.md");
    assert_eq!(section.provenance.heading_path, "Skills > Gun");
    assert_eq!(section.provenance.line_range.start, 7);
    assert_eq!(section.provenance.line_range.end, 8);
    let source = std::fs::read_to_string(fixture_vault().join("player.md")).unwrap();
    assert_eq!(
        &source[section.provenance.byte_range.start as usize
            ..section.provenance.byte_range.end as usize],
        section.content
    );
    assert_eq!(section.provenance.content_hash.len(), 64);
    assert!(
        section
            .provenance
            .content_hash
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    );
    let searched = index.search("gun skill");
    assert!(
        searched
            .iter()
            .all(|result| result.provenance.content_hash.len() == 64)
    );

    let error = index
        .get_section("player.md", "Skills > Cannon")
        .expect_err("unknown heading path");
    assert!(error.message.contains("Skills > Cannon"));
    assert!(error.suggestions.iter().any(|path| path == "Skills > Gun"));

    let error = index
        .get_section("missing.md", "Skills > Gun")
        .expect_err("unknown file");
    assert!(error.message.contains("missing.md"));
    assert!(error.suggestions.iter().any(|file| file == "player.md"));
}

#[test]
fn fuzzy_search_handles_case_order_partial_tokens_and_is_deterministic() {
    let index = VaultIndex::build(fixture_vault()).expect("fixture vault indexes");

    for query in ["gun skill", "skill gun", "Gun"] {
        let results = index.search(query);
        assert!(
            results
                .iter()
                .any(|result| result.provenance.file == "player.md"
                    && result.provenance.heading_path == "Skills > Gun"),
            "query {query:?} did not include the player gun section"
        );
    }

    let exact_path = index.search("Weapons > Gun Skill");
    assert_eq!(exact_path[0].provenance.heading_path, "Weapons > Gun Skill");
    assert_eq!(exact_path[0].score, 40_000);
    assert_eq!(index.search("gun skill"), index.search("gun skill"));
}
