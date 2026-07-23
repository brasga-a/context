#!/usr/bin/env python3
"""PostToolUse hook: auto-format a crate after Edit/Write touches its src/**."""
import json
import re
import subprocess
import sys

CRATE_SRC_RE = re.compile(r"/crates/([^/]+)/src/.*\.rs$")


def main() -> None:
    data = json.load(sys.stdin)
    file_path = data.get("tool_input", {}).get("file_path", "")
    project_dir = data.get("cwd") or "."

    match = CRATE_SRC_RE.search(file_path.replace("\\", "/"))
    if match:
        crate = match.group(1)
        subprocess.run(
            ["cargo", "fmt", "-p", crate],
            cwd=project_dir,
            capture_output=True,
        )

    sys.exit(0)


if __name__ == "__main__":
    main()
