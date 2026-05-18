#!/usr/bin/env bash
# Dispatch to the right per-distro bootstrap script.
# Reads /etc/os-release; falls back to a clear error if it can't.

set -euo pipefail

if [[ "$(uname -s)" == "Darwin" ]]; then
  exec "$(dirname "$0")/bootstrap-macos.sh" "$@"
fi

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "bootstrap.sh: unsupported OS '$(uname -s)'." >&2
  echo "Linux: run scripts/bootstrap-{debian,arch,fedora}.sh directly." >&2
  echo "macOS: scripts/bootstrap-macos.sh." >&2
  echo "Windows: scripts/bootstrap-windows.ps1 from PowerShell." >&2
  exit 1
fi

if [[ ! -r /etc/os-release ]]; then
  echo "bootstrap.sh: /etc/os-release missing or unreadable; cannot detect distro." >&2
  echo "Run scripts/bootstrap-{debian,arch,fedora}.sh directly." >&2
  exit 1
fi

# shellcheck disable=SC1091
. /etc/os-release

# Distro families. ID_LIKE may carry several space-separated tokens
# (e.g. "ubuntu debian" on Mint, "arch" on EndeavourOS, "fedora" on
# CentOS Stream); match against the union of ID + ID_LIKE.
family="${ID:-} ${ID_LIKE:-}"

script_dir="$(dirname "$0")"

case "$family" in
  *debian*|*ubuntu*)
    exec "$script_dir/bootstrap-debian.sh" "$@"
    ;;
  *arch*|*manjaro*|*cachyos*|*endeavouros*|*garuda*)
    exec "$script_dir/bootstrap-arch.sh" "$@"
    ;;
  *fedora*|*rhel*|*centos*|*rocky*|*almalinux*|*ol*|*amzn*)
    exec "$script_dir/bootstrap-fedora.sh" "$@"
    ;;
  *)
    echo "bootstrap.sh: didn't recognise distro family '$family'." >&2
    echo "Pick one of:" >&2
    echo "  scripts/bootstrap-debian.sh   (Debian, Ubuntu, Mint, Pop!_OS)" >&2
    echo "  scripts/bootstrap-arch.sh     (Arch, CachyOS, Manjaro, EndeavourOS, Garuda)" >&2
    echo "  scripts/bootstrap-fedora.sh   (Fedora, RHEL, Rocky, Alma, CentOS Stream)" >&2
    exit 1
    ;;
esac
