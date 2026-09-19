#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# OPad Host Installation Script for Linux (User layout ~/.local)
# Wraps top-level Makefile install-user target (B-1..B-4)
# -----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

echo "=== Installing OPad Desktop Suite (User layout) ==="
make -C "${REPO_ROOT}" install-user
