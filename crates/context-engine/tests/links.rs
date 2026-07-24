use std::fs;

use context_engine::VaultIndex;
use tempfile::TempDir;

fn temp_vault(files: &[(&str, &str)]) -> (TempDir, VaultIndex) {
    let vault = TempDir::new().expect("create temp vault");
    for (name, content) in files {
        let path = vault.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(path, content).expect("write fixture");
    }
    let index = VaultIndex::build(vault.path()).expect("index temp vault");
    (vault, index)
}

#[test]
fn backlinks_lists_every_resolved_link_and_narrows_by_heading_path() {
    let (_vault, index) = temp_vault(&[
        ("weapons.md", "## Gun Skill\nAdvanced handling.\n"),
        (
            "player.md",
            "## Notes\nSee [[weapons]] and [[weapons#Gun Skill]].\n",
        ),
    ]);

    let all = index
        .backlinks("weapons.md", None)
        .expect("weapons.md is indexed");
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|link| link.target_heading_path.is_none()));
    assert!(
        all.iter()
            .any(|link| link.target_heading_path.as_deref() == Some("Gun Skill"))
    );
    assert!(all.iter().all(|link| link.from.file == "player.md"));
    assert!(all.iter().all(|link| link.from.heading_path == "Notes"));
    assert!(all.iter().all(|link| !link.from.content_hash.is_empty()));

    let narrowed = index
        .backlinks("weapons.md", Some("Gun Skill"))
        .expect("narrowed lookup");
    assert_eq!(narrowed.len(), 1);
    assert_eq!(narrowed[0].raw_target, "weapons#Gun Skill");
}

#[test]
fn unresolved_links_are_diagnostics_not_failures() {
    let (_vault, index) = temp_vault(&[(
        "player.md",
        "## Notes\nBroken: [[Nowhere]] and [[#AlsoNowhere]].\n",
    )]);

    assert!(
        index
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Nowhere"))
    );
    assert!(
        index
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("AlsoNowhere"))
    );
    // Unresolved links never fail the build; get_section still works normally.
    assert!(index.get_section("player.md", "Notes").is_ok());
}

#[test]
fn unknown_backlinks_file_is_a_helpful_not_found_error() {
    let (_vault, index) = temp_vault(&[("player.md", "# Player\nbody\n")]);
    let error = index
        .backlinks("missing.md", None)
        .expect_err("unknown file");
    assert!(error.message.contains("missing.md"));
}

#[test]
fn editing_one_file_flips_another_files_link_resolution_without_touching_it() {
    let (_vault, mut index) = temp_vault(&[
        ("a.md", "# A\nintro\n"),
        ("b.md", "## Notes\nSee [[a#Widget]].\n"),
    ]);

    // Before the edit: B's link to A#Widget does not resolve.
    assert!(
        index
            .backlinks("a.md", None)
            .expect("a.md indexed")
            .is_empty()
    );
    assert!(
        index
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Widget"))
    );

    let hash = index
        .get_section("a.md", "A")
        .expect("read section A")
        .provenance
        .content_hash;
    index
        .edit_section("a.md", "A", "intro\n\n### Widget\nnew child section", &hash)
        .expect("edit adds a child heading");

    // After the edit: B's link now resolves, even though B itself was never re-read or edited.
    let backlinks = index.backlinks("a.md", Some("A > Widget")).expect("lookup");
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0].from.file, "b.md");
    assert!(
        !index
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Widget"))
    );
}
