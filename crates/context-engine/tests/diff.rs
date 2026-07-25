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
fn reindex_vault_reflects_an_externally_added_file() {
    let (vault, mut index) = temp_vault(&[("a.md", "# A\nbody\n")]);
    fs::write(vault.path().join("b.md"), "# B\nbody\n").expect("write new file");

    let diff = index.reindex_vault().expect("reindex succeeds");
    assert_eq!(diff.files_added, ["b.md"]);
    assert!(diff.files_removed.is_empty());
    assert!(diff.files_changed.is_empty());
    assert_eq!(index.files().collect::<Vec<_>>(), ["a.md", "b.md"]);
}

#[test]
fn reindex_vault_reflects_an_externally_removed_file() {
    let (vault, mut index) = temp_vault(&[("a.md", "# A\nbody\n"), ("b.md", "# B\nbody\n")]);
    fs::remove_file(vault.path().join("b.md")).expect("remove file");

    let diff = index.reindex_vault().expect("reindex succeeds");
    assert_eq!(diff.files_removed, ["b.md"]);
    assert!(diff.files_added.is_empty());
    assert_eq!(index.files().collect::<Vec<_>>(), ["a.md"]);
}

#[test]
fn reindex_vault_reflects_an_externally_edited_section() {
    let (vault, mut index) = temp_vault(&[("a.md", "## Skills\n\n### Gun\nold body\n")]);
    fs::write(
        vault.path().join("a.md"),
        "## Skills\n\n### Gun\nnew body\n",
    )
    .expect("edit file externally");

    let diff = index.reindex_vault().expect("reindex succeeds");
    assert_eq!(diff.files_changed.len(), 1);
    let file_diff = &diff.files_changed[0];
    assert_eq!(file_diff.file, "a.md");
    assert_eq!(file_diff.sections_modified, ["Skills > Gun"]);
    assert!(file_diff.sections_added.is_empty());
    assert!(file_diff.sections_removed.is_empty());

    assert_eq!(
        index.get_section("a.md", "Skills > Gun").unwrap().content,
        "### Gun\nnew body"
    );
}

#[test]
fn reindex_vault_reflects_an_externally_added_nested_section() {
    let (vault, mut index) = temp_vault(&[("a.md", "# A\nintro\n")]);
    fs::write(
        vault.path().join("a.md"),
        "# A\nintro\n\n## Notes\ntext\n\n### Sub\nchild\n",
    )
    .expect("add nested sections externally");

    let diff = index.reindex_vault().expect("reindex succeeds");
    let file_diff = &diff.files_changed[0];
    assert_eq!(file_diff.sections_added, ["A > Notes"]);
}

#[test]
fn reindex_vault_reports_no_change_for_an_untouched_vault() {
    let (_vault, mut index) = temp_vault(&[("a.md", "# A\nbody\n")]);
    let diff = index.reindex_vault().expect("reindex succeeds");
    assert!(diff.files_added.is_empty());
    assert!(diff.files_removed.is_empty());
    assert!(diff.files_changed.is_empty());
}

#[test]
fn reindex_vault_fails_and_leaves_the_index_untouched_when_the_root_is_gone() {
    let (vault, mut index) = temp_vault(&[("a.md", "# A\nbody\n")]);
    let root = vault.path().to_path_buf();
    drop(vault);
    std::fs::remove_dir_all(&root).ok();

    let error = index.reindex_vault().expect_err("missing root fails");
    assert!(error.to_string().contains(&root.display().to_string()));
    // The previous index is left exactly as it was.
    assert_eq!(index.files().collect::<Vec<_>>(), ["a.md"]);
    assert_eq!(index.get_section("a.md", "A").unwrap().content, "# A\nbody");
}

#[test]
fn reindex_vault_refreshes_backlinks_across_files() {
    let (vault, mut index) = temp_vault(&[
        ("a.md", "# A\nintro\n"),
        ("b.md", "## Notes\nSee [[a#Widget]].\n"),
    ]);
    assert!(index.backlinks("a.md", None).unwrap().is_empty());

    fs::write(
        vault.path().join("a.md"),
        "# A\nintro\n\n### Widget\nnew child section\n",
    )
    .expect("add heading externally");

    index.reindex_vault().expect("reindex succeeds");
    let backlinks = index.backlinks("a.md", Some("A > Widget")).unwrap();
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0].from.file, "b.md");
}
