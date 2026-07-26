#!/usr/bin/env sh
# SPDX-License-Identifier: GPL-3.0-only
#
# Proves the 64 KiB argv length cap in
# `enforce_argv_budget` (see crates/priv-helper/src/main.rs).
# A single 100 KiB argument is rejected with exit code 2
# *before* clap ever sees the request.
#
# Unix only — Windows CreateProcess refuses commandlines past
# ~32 KiB at the kernel level before our cap can run, so the
# Windows test in `tests/cli.rs` is cfg-gated away.

set -euo pipefail

if [ "$(uname -s)" = "MINGW"* ] || [ "$(uname -s)" = "MSYS"* ]; then
    echo "Skipping on Windows-like host." >&2
    exit 0
fi

big=$(printf 'x%.0s' $(seq 1 100000))

echo "Building a single argument of $(printf %s "$big" | wc -c) bytes…"
echo "$ raidhos-priv-helper \"\$big\""
if raidhos-priv-helper "$big" 2>/dev/null; then
    echo "Unexpected: helper accepted oversized argv." >&2
    exit 1
fi
rc=$?
if [ "$rc" -eq 2 ]; then
    echo "Helper correctly rejected oversized argv (exit 2)."
else
    echo "Unexpected exit code $rc." >&2
    exit 1
fi
