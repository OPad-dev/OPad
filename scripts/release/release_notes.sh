#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# Prints the "Which file do I download?" table that starts an app release's
# notes (publish_release.sh puts it there), from the files about to be
# uploaded, so it only names files the release really has.
#
#   release_notes.sh TAG DIR
#
# The names are release-app.yml's: OPad-Windows-Setup-<v>.exe,
# OPad-Linux-Debian-Ubuntu-<v>.deb, OPad-Linux-Fedora-<v>.rpm,
# OPad-Linux-Arch-<v>.pkg.tar.zst, OPad-Linux-<v>.AppImage, Source-*.
# Needs GITHUB_REPOSITORY in the environment.
# -----------------------------------------------------------------------------

TAG="$1"
DIR="$2"
BASE="https://github.com/${GITHUB_REPOSITORY}/releases/download/${TAG}"

# A markdown link to the one file in DIR matching the glob, or nothing
link() {
    local f
    shopt -s nullglob
    for f in "${DIR}"/$1; do
        f="$(basename "$f")"
        printf '[`%s`](%s/%s)' "$f" "$BASE" "$f"
        return
    done
}

row() {
    local file
    file="$(link "$2")"
    [ -n "$file" ] && printf '| %s | %s%s |\n' "$1" "$file" "${3:-}"
    return 0
}

echo "### Which file do I download?"
echo
echo "| Your computer | Download |"
echo "|---|---|"
row "Windows 10 or 11" "OPad-Windows-Setup-*.exe"
row "Ubuntu or Debian" "OPad-Linux-Debian-Ubuntu-*.deb"
row "Fedora" "OPad-Linux-Fedora-*.rpm"
row "Arch Linux" "OPad-Linux-Arch-*.pkg.tar.zst" " (\`sudo pacman -U\` it)"
row "Any other Linux" "OPad-Linux-*.AppImage"
echo
echo "Once installed, OPad keeps itself and your pad's firmware up to date."
echo "The \`Source-*\` files are the source code of the bundled tosu, published"
echo "as its licence asks; you do not need them. GitHub shows each file's"
echo "SHA-256 next to it."
