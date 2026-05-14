#!/usr/bin/env python3
"""Patch release.yml after `dist generate` to restore scoped GitHub workflow permissions.

dist generates `permissions: contents: write` at the top level; Scorecard flags this.
This patches it to `contents: read` and grants `contents: write` only to the two jobs
that actually create GitHub Releases (plan and host).
"""

path = ".github/workflows/release.yml"
with open(path) as f:
    content = f.read()

content = content.replace(
    'name: Release\npermissions:\n  "contents": "write"',
    'name: Release\npermissions:\n  "contents": "read"',
)
content = content.replace(
    '  plan:\n    runs-on:',
    '  plan:\n    permissions:\n      "contents": "write"\n    runs-on:',
)
content = content.replace(
    '  host:\n    needs:',
    '  host:\n    permissions:\n      "contents": "write"\n    needs:',
)

with open(path, "w") as f:
    f.write(content)

print("Patched release.yml: scoped contents:write to plan and host jobs only")
