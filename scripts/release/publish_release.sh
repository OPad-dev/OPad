#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# Uploads a directory of built files to a GitHub release and publishes it.
#
#   publish_release.sh TAG DIR PRERELEASE LATEST
#
#   TAG         the release tag (v1.2.0, firmware-v1.2.0, tosu-v4.26.2-r1)
#   DIR         every file in it is uploaded, replacing a file of the same name
#   PRERELEASE  true | false
#   LATEST      latest-if-final: GitHub's "Latest" when not a pre-release (the
#                                app only, docs/specs/release-and-updates.md §4.1)
#               never:           firmware and tosu
#
# The release is normally the draft release-please created with the tag. A
# release started by hand may have neither: the draft is then created here,
# and the tag too when TARGET is set (a commit SHA), as release-tosu.yml does.
# Needs gh with GH_TOKEN, and GITHUB_REPOSITORY in the environment.
# -----------------------------------------------------------------------------

TAG="$1"
DIR="$2"
PRERELEASE="$3"
LATEST="$4"

case "${PRERELEASE}" in true|false) ;; *) echo "PRERELEASE must be true or false" >&2; exit 1 ;; esac
case "${LATEST}" in
    latest-if-final) [ "${PRERELEASE}" = false ] && MAKE_LATEST=true || MAKE_LATEST=false ;;
    never) MAKE_LATEST=false ;;
    *) echo "LATEST must be latest-if-final or never" >&2; exit 1 ;;
esac

shopt -s nullglob
files=("${DIR}"/*)
if [ ${#files[@]} -eq 0 ]; then
    echo "ERROR: nothing to upload in ${DIR}" >&2
    exit 1
fi

if ! gh release view "${TAG}" >/dev/null 2>&1; then
    echo "--- No release for ${TAG} yet: creating a draft ---"
    create_args=(--draft --title "${TAG}" --generate-notes)
    if [ -n "${TARGET:-}" ]; then
        create_args+=(--target "${TARGET}")
    else
        create_args+=(--verify-tag)
    fi
    gh release create "${TAG}" "${create_args[@]}"
fi

echo "--- Uploading ${#files[@]} file(s) to ${TAG} ---"
gh release upload "${TAG}" --clobber "${files[@]}"

echo "--- Publishing ${TAG} (pre-release: ${PRERELEASE}, latest: ${MAKE_LATEST}) ---"
gh release edit "${TAG}" --draft=false --prerelease="${PRERELEASE}" --latest="${MAKE_LATEST}"
gh release view "${TAG}" --json url,isDraft,isPrerelease -q '"\(.url) draft=\(.isDraft) prerelease=\(.isPrerelease)"'
