#!/usr/bin/env python3
"""Stop hook: non-blocking reminder if a crate's src/** changed without its CHANGELOG.md."""
import json
import re
import subprocess
import sys

SRC_RE = re.compile(r"^crates/([^/]+)/src/")
CHANGELOG_RE = re.compile(r"^crates/([^/]+)/CHANGELOG\.md$")


def main() -> None:
    data = json.load(sys.stdin)
    project_dir = data.get("cwd") or "."

    result = subprocess.run(
        ["git", "status", "--porcelain", "--untracked-files=all"],
        cwd=project_dir,
        capture_output=True,
        text=True,
    )
    changed_files = [line[3:] for line in result.stdout.splitlines() if line.strip()]

    touched_src = set()
    touched_changelog = set()
    for path in changed_files:
        src_match = SRC_RE.match(path)
        if src_match:
            touched_src.add(src_match.group(1))
        changelog_match = CHANGELOG_RE.match(path)
        if changelog_match:
            touched_changelog.add(changelog_match.group(1))

    missing = sorted(touched_src - touched_changelog)
    if missing:
        crates_list = ", ".join(missing)
        message = (
            f"Reminder: source changed in {crates_list} but the matching CHANGELOG.md "
            "wasn't updated. Add a Keep-a-Changelog ## [Unreleased] entry "
            "(Added/Changed/Fixed + SemVer impact) if this is a real behavior change."
        )
        print(json.dumps({"hookSpecificOutput": {"additionalContext": message}}))

    sys.exit(0)


if __name__ == "__main__":
    main()
