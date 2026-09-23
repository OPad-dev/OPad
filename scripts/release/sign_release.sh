#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# Signs the release manifest over the exact files a release publishes (§U-0.3)
#
#   sign_release.sh --tag vX.Y.Z [--repo OWNER/REPO] [--no-verify-run] [-- opad-manifest args]
#       Downloads every asset of the DRAFT release release.yml created, builds
#       and signs opad-manifest.json over those files, checks it, uploads the
#       manifest and its .minisig to the draft, then starts release-verify.yml,
#       which checks again on GitHub and publishes the draft.
#
#   sign_release.sh --dist DIR [--no-sign] [-- opad-manifest args]
#       Builds (and signs) the manifest over an existing directory in place,
#       for a local release or a dry run. Nothing is downloaded or uploaded.
#
# Nothing is rebuilt and nothing is deleted from the input: the manifest must
# name the bytes users download, not a local rebuild of them. The signing key
# stays on this machine (~/.config/opad/opad-manifest.key, or --secret-key
# after `--`); CI never sees it.
# -----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

TAG=""
REPO=""
DIST=""
SIGN=1
VERIFY_RUN=1
EXTRA=()

usage() {
    sed -n '5,21p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

while [ $# -gt 0 ]; do
    case "$1" in
        --tag) TAG="$2"; shift 2 ;;
        --repo) REPO="$2"; shift 2 ;;
        --dist) DIST="$2"; shift 2 ;;
        --no-sign) SIGN=0; shift ;;
        --no-verify-run) VERIFY_RUN=0; shift ;;
        -h|--help) usage 0 ;;
        --) shift; EXTRA=("$@"); break ;;
        *) echo "Unknown argument: $1" >&2; usage 1 ;;
    esac
done

if { [ -n "${TAG}" ] && [ -n "${DIST}" ]; } || { [ -z "${TAG}" ] && [ -z "${DIST}" ]; }; then
    echo "Give exactly one of --tag or --dist" >&2
    usage 1
fi
if [ -n "${TAG}" ] && [ "${SIGN}" = 0 ]; then
    echo "--no-sign is for --dist dry runs; a draft release needs a signed manifest" >&2
    exit 1
fi

REPO_ARGS=()
[ -n "${REPO}" ] && REPO_ARGS=(--repo "${REPO}")

BASE_URL="${BASE_URL:-https://github.com/OPad-dev/OPad/releases/latest/download}"
FW_VER="${FIRMWARE_VERSION:-$(sed -n 's/^set(PROJECT_VER "\(.*\)")$/\1/p' "${REPO_ROOT}/firmware/CMakeLists.txt" | head -n 1)}"
if [ -z "${FW_VER}" ]; then
    echo "ERROR: no firmware version (set FIRMWARE_VERSION)" >&2
    exit 1
fi

if [ -n "${TAG}" ]; then
    command -v gh >/dev/null 2>&1 || { echo "ERROR: gh (GitHub CLI) is required" >&2; exit 1; }
    if [ "$(gh release view "${TAG}" "${REPO_ARGS[@]}" --json isDraft -q .isDraft)" != "true" ]; then
        echo "ERROR: ${TAG} is not a draft release; publishing happens only after signing" >&2
        exit 1
    fi
    DIST="$(mktemp -d "${TMPDIR:-/tmp}/opad-release-${TAG}.XXXXXX")"
    echo "--- Downloading the assets of draft ${TAG} into ${DIST} ---"
    gh release download "${TAG}" "${REPO_ARGS[@]}" --dir "${DIST}"
    # A manifest from an earlier signing attempt is replaced, never an input
    rm -f "${DIST}/opad-manifest.json" "${DIST}/opad-manifest.json.minisig"
fi

if [ ! -d "${DIST}" ]; then
    echo "ERROR: ${DIST} is not a directory" >&2
    exit 1
fi

echo ""
echo "--- Building the release manifest over ${DIST} ---"
SIGN_ARGS=()
[ "${SIGN}" = 0 ] && SIGN_ARGS=(--no-sign)
cargo run --quiet --manifest-path "${REPO_ROOT}/desktop/Cargo.toml" -p opad-update --bin opad-manifest -- \
    --dist "${DIST}" \
    --base-url "${BASE_URL}" \
    --firmware-version "${FW_VER}" \
    "${SIGN_ARGS[@]}" \
    "${EXTRA[@]}"

VERIFY_ARGS=()
[ "${SIGN}" = 1 ] && VERIFY_ARGS=(--verify-signature)
python3 "${SCRIPT_DIR}/check_manifest.py" "${DIST}" "${VERIFY_ARGS[@]}"

if [ -z "${TAG}" ]; then
    echo "✓ Manifest written in ${DIST}"
    exit 0
fi

echo ""
echo "--- Uploading the manifest to draft ${TAG} ---"
gh release upload "${TAG}" "${REPO_ARGS[@]}" --clobber \
    "${DIST}/opad-manifest.json" "${DIST}/opad-manifest.json.minisig"

if [ "${VERIFY_RUN}" = 1 ]; then
    gh workflow run release-verify.yml "${REPO_ARGS[@]}" -f tag="${TAG}"
    echo "✓ Started release-verify.yml: it re-checks the draft's files on GitHub and publishes ${TAG}"
else
    echo "✓ Uploaded. Run release-verify.yml with tag=${TAG} to check and publish the draft."
fi
