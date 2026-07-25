use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::EngineDocument;
use crate::vault::{VaultError, VaultIndex, visit_sections};

/// What changed in a vault between two `VaultIndex` snapshots.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct VaultDiff {
    /// Vault-relative paths of files present after but not before, sorted.
    pub files_added: Vec<String>,
    /// Vault-relative paths of files present before but not after, sorted.
    pub files_removed: Vec<String>,
    /// Files present both before and after with at least one section-level difference.
    pub files_changed: Vec<FileDiff>,
}

/// Section-level differences within one file present both before and after a reindex.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FileDiff {
    pub file: String,
    /// New heading paths; only the topmost heading of each newly added subtree.
    pub sections_added: Vec<String>,
    /// Gone heading paths; only the topmost heading of each removed subtree.
    pub sections_removed: Vec<String>,
    /// Heading paths whose `content_hash` differs, excluding any with a changed descendant.
    pub sections_modified: Vec<String>,
}

impl VaultIndex {
    /// Re-walks the vault root, diffs the result against the current index, and replaces it.
    ///
    /// Fails without diffing or replacing the current index if the root is no longer a valid
    /// directory (mirrors `VaultIndex::build`'s own validation) — a missing root must never be
    /// reported as "every file was removed."
    pub fn reindex_vault(&mut self) -> Result<VaultDiff, VaultError> {
        let new_index = VaultIndex::build(&self.root)?;
        let diff = diff_vaults(self, &new_index);
        *self = new_index;
        Ok(diff)
    }
}

fn diff_vaults(old: &VaultIndex, new: &VaultIndex) -> VaultDiff {
    let old_files: BTreeSet<&str> = old.documents.keys().map(String::as_str).collect();
    let new_files: BTreeSet<&str> = new.documents.keys().map(String::as_str).collect();

    let files_added = new_files
        .difference(&old_files)
        .map(|file| (*file).to_owned())
        .collect();
    let files_removed = old_files
        .difference(&new_files)
        .map(|file| (*file).to_owned())
        .collect();

    let mut files_changed = Vec::new();
    for file in old_files.intersection(&new_files) {
        let old_document = &old.documents[*file];
        let new_document = &new.documents[*file];
        if let Some(diff) = diff_file(file, old_document, new_document) {
            files_changed.push(diff);
        }
    }

    VaultDiff {
        files_added,
        files_removed,
        files_changed,
    }
}

fn diff_file(file: &str, old: &EngineDocument, new: &EngineDocument) -> Option<FileDiff> {
    let old_flat = flatten(&old.sections);
    let new_flat = flatten(&new.sections);

    let old_paths: BTreeSet<&str> = old_flat.keys().map(String::as_str).collect();
    let new_paths: BTreeSet<&str> = new_flat.keys().map(String::as_str).collect();

    let added: BTreeSet<&str> = new_paths.difference(&old_paths).copied().collect();
    let removed: BTreeSet<&str> = old_paths.difference(&new_paths).copied().collect();
    let modified_candidates: BTreeSet<&str> = old_paths
        .intersection(&new_paths)
        .copied()
        .filter(|path| old_flat[*path] != new_flat[*path])
        .collect();

    if added.is_empty() && removed.is_empty() && modified_candidates.is_empty() {
        return None;
    }

    let sections_added = shallowest_only(&added);
    let sections_removed = shallowest_only(&removed);
    let all_changed: BTreeSet<&str> = added
        .iter()
        .chain(removed.iter())
        .chain(modified_candidates.iter())
        .copied()
        .collect();
    let sections_modified = deepest_only(&modified_candidates, &all_changed);

    Some(FileDiff {
        file: file.to_owned(),
        sections_added,
        sections_removed,
        sections_modified,
    })
}

/// Flattens a section tree into `heading_path -> content_hash`.
fn flatten(sections: &[crate::Section]) -> BTreeMap<String, String> {
    let mut flat = BTreeMap::new();
    visit_sections(sections, &mut |section| {
        flat.insert(section.heading_path.clone(), section.content_hash.clone());
    });
    flat
}

/// Keeps only paths whose parent is not itself in `paths` — the top of each changed subtree.
fn shallowest_only(paths: &BTreeSet<&str>) -> Vec<String> {
    paths
        .iter()
        .filter(|path| !has_ancestor_in(path, paths))
        .map(|path| (*path).to_owned())
        .collect()
}

/// Keeps only paths from `candidates` with no descendant in `all_changed` — the bottom of each
/// hash-cascade chain.
fn deepest_only(candidates: &BTreeSet<&str>, all_changed: &BTreeSet<&str>) -> Vec<String> {
    candidates
        .iter()
        .filter(|path| !has_descendant_in(path, all_changed))
        .map(|path| (*path).to_owned())
        .collect()
}

fn has_ancestor_in(path: &str, paths: &BTreeSet<&str>) -> bool {
    paths
        .iter()
        .any(|other| *other != path && path.starts_with(&format!("{other} > ")))
}

fn has_descendant_in(path: &str, paths: &BTreeSet<&str>) -> bool {
    let prefix = format!("{path} > ");
    paths
        .iter()
        .any(|other| *other != path && other.starts_with(&prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(source: &str) -> EngineDocument {
        EngineDocument::parse(source)
    }

    #[test]
    fn flatten_collects_every_heading_path_and_hash() {
        let doc = document("# Player\nlead\n\n## Skills\n\n### Gun\nbody");
        let flat = flatten(&doc.sections);
        assert_eq!(flat.len(), 3);
        assert!(flat.contains_key("Player"));
        assert!(flat.contains_key("Player > Skills"));
        assert!(flat.contains_key("Player > Skills > Gun"));
    }

    #[test]
    fn flatten_includes_the_preamble() {
        let doc = document("intro\n\n# Head\nbody");
        let flat = flatten(&doc.sections);
        assert!(flat.contains_key("Preamble"));
    }

    #[test]
    fn diff_file_reports_added_removed_and_modified_sections() {
        let old = document("## Skills\nintro\n\n### Gun\nold body\n\n## Inventory\nitems");
        let new = document("## Skills\nintro\n\n### Gun\nnew body\n\n## Notes\ntext");

        let diff = diff_file("player.md", &old, &new).expect("file changed");
        assert_eq!(diff.file, "player.md");
        assert_eq!(diff.sections_added, ["Notes"]);
        assert_eq!(diff.sections_removed, ["Inventory"]);
        assert_eq!(diff.sections_modified, ["Skills > Gun"]);
    }

    #[test]
    fn diff_file_returns_none_when_nothing_changed() {
        let old = document("## Skills\n\n### Gun\nbody");
        let new = document("## Skills\n\n### Gun\nbody");
        assert_eq!(diff_file("player.md", &old, &new), None);
    }

    #[test]
    fn root_cause_filtering_suppresses_cascading_modified_ancestors() {
        let old = document("# Player\n\n## Skills\n\n### Gun\nold");
        let new = document("# Player\n\n## Skills\n\n### Gun\nnew");

        let diff = diff_file("player.md", &old, &new).expect("changed");
        // Skills and Player both changed hash (cascade), but only Gun is the real edit.
        assert_eq!(diff.sections_modified, ["Player > Skills > Gun"]);
    }

    #[test]
    fn root_cause_filtering_reports_only_the_topmost_new_subtree_heading() {
        let old = document("# Player\nintro");
        let new = document("# Player\nintro\n\n## Notes\ntext\n\n### Sub\nchild");

        let diff = diff_file("player.md", &old, &new).expect("changed");
        assert_eq!(diff.sections_added, ["Player > Notes"]);
    }

    #[test]
    fn root_cause_filtering_reports_only_the_topmost_removed_subtree_heading() {
        let old = document("# Player\nintro\n\n## Notes\ntext\n\n### Sub\nchild");
        let new = document("# Player\nintro");

        let diff = diff_file("player.md", &old, &new).expect("changed");
        assert_eq!(diff.sections_removed, ["Player > Notes"]);
    }

    #[test]
    fn rename_is_reported_as_delete_plus_add_never_as_a_rename() {
        let old = document("## Gun\nbody");
        let new = document("## Rifle\nbody");

        let diff = diff_file("player.md", &old, &new).expect("changed");
        assert_eq!(diff.sections_added, ["Rifle"]);
        assert_eq!(diff.sections_removed, ["Gun"]);
        assert!(diff.sections_modified.is_empty());
    }
}
