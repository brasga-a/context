use std::fs;

use context_engine::{EditError, VaultIndex};
use tempfile::TempDir;

const PLAYER: &str = "# Player\nlead\n\n## Skills\nskill intro\n\n### Gun\nFire the equipped weapon.\n\n## Inventory\nitems\n";

fn temp_vault() -> (TempDir, VaultIndex) {
    let vault = TempDir::new().expect("create temp vault");
    fs::write(vault.path().join("player.md"), PLAYER).expect("write fixture");
    let index = VaultIndex::build(vault.path()).expect("index temp vault");
    (vault, index)
}

fn gun_hash(index: &VaultIndex) -> String {
    index
        .get_section("player.md", "Player > Skills > Gun")
        .expect("gun section")
        .provenance
        .content_hash
}

#[test]
fn a_matching_hash_edits_exactly_one_body_and_refreshes_the_index() {
    let (vault, mut index) = temp_vault();
    let hash = gun_hash(&index);

    let outline = index
        .edit_section(
            "player.md",
            "Player > Skills > Gun",
            "Deal 3 damage.",
            &hash,
        )
        .expect("edit succeeds");

    let written = fs::read_to_string(vault.path().join("player.md")).expect("read back");
    assert_eq!(
        written,
        "# Player\nlead\n\n## Skills\nskill intro\n\n### Gun\nDeal 3 damage.\n\n## Inventory\nitems\n"
    );

    let section = index
        .get_section("player.md", "Player > Skills > Gun")
        .expect("index refreshed");
    assert_eq!(section.content, "### Gun\nDeal 3 damage.");

    // The returned outline carries usable guard tokens for a follow-up edit.
    let gun = &outline[0].children[0].children[0];
    assert_eq!(gun.heading_path, "Player > Skills > Gun");
    index
        .edit_section(
            "player.md",
            "Player > Skills > Gun",
            "Deal 4 damage.",
            &gun.content_hash,
        )
        .expect("chained edit with returned hash succeeds");
}

#[test]
fn a_stale_hash_is_a_conflict_and_the_file_is_untouched() {
    let (vault, mut index) = temp_vault();

    let error = index
        .edit_section(
            "player.md",
            "Player > Skills > Gun",
            "new body",
            "0".repeat(64).as_str(),
        )
        .expect_err("stale hash conflicts");
    let EditError::Conflict { current_hash, .. } = &error else {
        panic!("expected conflict, got {error:?}");
    };
    assert_eq!(current_hash, &gun_hash(&index));
    assert_eq!(
        fs::read_to_string(vault.path().join("player.md")).expect("read back"),
        PLAYER
    );
}

#[test]
fn disk_content_trumps_a_stale_index() {
    let (vault, mut index) = temp_vault();
    let external = PLAYER.replace("Fire the equipped weapon.", "Externally changed.");
    fs::write(vault.path().join("player.md"), &external).expect("external edit");

    // The old in-memory hash no longer authorizes an edit...
    let stale = index
        .edit_section(
            "player.md",
            "Player > Skills > Gun",
            "new body",
            &gun_hash(&index),
        )
        .expect_err("stale in-memory hash conflicts");
    assert!(matches!(stale, EditError::Conflict { .. }));

    // ...but the current on-disk hash does, even though the index never saw that content.
    let disk_hash = {
        let fresh = VaultIndex::build(vault.path()).expect("fresh index");
        gun_hash(&fresh)
    };
    index
        .edit_section("player.md", "Player > Skills > Gun", "new body", &disk_hash)
        .expect("disk-current hash succeeds");
}

#[test]
fn unknown_targets_fail_helpfully_and_traversal_is_rejected() {
    let (_vault, mut index) = temp_vault();

    let error = index
        .edit_section("missing.md", "Player", "body", "hash")
        .expect_err("unknown file");
    let EditError::NotFound(not_found) = &error else {
        panic!("expected not-found, got {error:?}");
    };
    assert!(not_found.message.contains("missing.md"));
    assert!(not_found.suggestions.iter().any(|file| file == "player.md"));

    let error = index
        .edit_section("player.md", "Player > Skills > Cannon", "body", "hash")
        .expect_err("unknown heading path");
    let EditError::NotFound(not_found) = &error else {
        panic!("expected not-found, got {error:?}");
    };
    assert!(not_found.message.contains("Skills > Cannon"));
    assert!(
        not_found
            .suggestions
            .iter()
            .any(|path| path == "Player > Skills > Gun")
    );

    let error = index
        .edit_section("../outside.md", "Player", "body", "hash")
        .expect_err("traversal rejected");
    assert!(matches!(error, EditError::NotFound(_)));
}

#[test]
fn rejected_edits_leave_file_and_index_untouched() {
    let (vault, mut index) = temp_vault();
    let hash = gun_hash(&index);

    let error = index
        .edit_section(
            "player.md",
            "Player > Skills > Gun",
            "## Escape Attempt",
            &hash,
        )
        .expect_err("escape rejected");
    assert!(matches!(error, EditError::Escape { .. }));
    assert!(error.message().contains("'Escape Attempt'"));

    assert_eq!(
        fs::read_to_string(vault.path().join("player.md")).expect("read back"),
        PLAYER
    );
    assert_eq!(
        index
            .get_section("player.md", "Player > Skills > Gun")
            .expect("index unchanged")
            .content,
        "### Gun\nFire the equipped weapon."
    );
}

#[test]
fn the_preamble_is_not_editable() {
    let vault = TempDir::new().expect("create temp vault");
    fs::write(vault.path().join("note.md"), "intro text\n\n# Head\nbody\n").expect("write");
    let mut index = VaultIndex::build(vault.path()).expect("index");

    let error = index
        .edit_section("note.md", "Preamble", "new intro", "hash")
        .expect_err("preamble rejected");
    assert!(matches!(error, EditError::InvalidTarget { .. }));
}

#[test]
fn reindex_file_refreshes_a_stale_entry() {
    let (vault, mut index) = temp_vault();
    let external = PLAYER.replace("Fire the equipped weapon.", "Externally changed.");
    fs::write(vault.path().join("player.md"), &external).expect("external edit");

    assert_eq!(
        index
            .get_section("player.md", "Player > Skills > Gun")
            .expect("stale read")
            .content,
        "### Gun\nFire the equipped weapon."
    );
    index.reindex_file("player.md").expect("reindex");
    assert_eq!(
        index
            .get_section("player.md", "Player > Skills > Gun")
            .expect("fresh read")
            .content,
        "### Gun\nExternally changed."
    );
}
