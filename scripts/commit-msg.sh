#!/usr/bin/env bash
# Validate the commit message follows Conventional Commits.
# Usage: commit-msg.sh <path-to-commit-msg-file>
set -euo pipefail

MSG_FILE="${1:?usage: commit-msg.sh <commit-msg-file>}"
PATTERN='^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert|release)(\([a-zA-Z0-9._/-]+\))?!?: .{1,100}'

# Skip merge / revert / fixup commits entirely.
if grep -qiE '^(merge |revert |fixup!|squash!)' "$MSG_FILE"; then
  exit 0
fi

if ! grep -qE "$PATTERN" "$MSG_FILE"; then
  cat >&2 <<'EOF'
Commit message does not follow Conventional Commits:
  <type>(<optional scope>)<!?: <description>

Examples:
  feat(ffi): add generational handle pool
  fix!: correct zero-copy lifetime handling
  docs: update DESIGN.md

Allowed types: feat fix docs style refactor perf test build ci chore revert release
EOF
  exit 1
fi
