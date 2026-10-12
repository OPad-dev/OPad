#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# Tests the apt and dnf repositories before they are published
# (docs/specs/release-and-updates.md §10.1, §12).
#
#   scripts/release/test_repos.sh [DIR]     DIR defaults to dist-local/
#
# Builds both repositories over the .deb/.rpm in DIR with build_repos.sh,
# signed by a throwaway GPG key made here, serves them on this machine, and in
# clean containers adds the repository the way the install instructions say,
# installs OPad, and checks the signature was required (a repository signed
# by another key must be refused). Needs: docker, gpg, apt-ftparchive,
# createrepo_c, python3.
# -----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DIR="${1:-${REPO_ROOT}/dist-local}"
PORT="${PORT:-8765}"

WORK="$(mktemp -d)"
cleanup() {
    [ -n "${SERVER:-}" ] && kill "${SERVER}" 2>/dev/null || true
    rm -rf "${WORK}"
}
trap cleanup EXIT

export GNUPGHOME="${WORK}/gnupg"
mkdir -m 700 "${GNUPGHOME}"
gpg --batch --quiet --passphrase '' --quick-generate-key "OPad repo test <test@invalid>" rsa3072 sign 1d
export GPG_KEY_ID="$(gpg --list-secret-keys --with-colons | awk -F: '/^fpr/ {print $10; exit}')"

mkdir -p "${WORK}/pkgs"
cp "${DIR}"/*.deb "${DIR}"/*.rpm "${WORK}/pkgs/"
# The release signs its .rpm (release-app.yml, §19.3); gpgcheck=1 needs it
command -v rpmsign >/dev/null || { echo "ERROR: rpmsign is required (rpm-tools / rpm-sign)" >&2; exit 1; }
rpmsign --define "_gpg_name ${GPG_KEY_ID}" --addsign "${WORK}"/pkgs/*.rpm
BASE_URL="http://127.0.0.1:${PORT}" "${SCRIPT_DIR}/build_repos.sh" "${WORK}/site" beta="${WORK}/pkgs"

python3 -m http.server "${PORT}" --bind 127.0.0.1 --directory "${WORK}/site" >/dev/null 2>&1 &
SERVER=$!
sleep 1

fail=0
for image in ubuntu:22.04 ubuntu:24.04 debian:12; do
    echo "=== ${image}: apt ==="
    if docker run --rm --network host -v "${WORK}/site:/site:ro" "${image}" bash -euc "
        apt-get update -qq && apt-get install -y -qq ca-certificates >/dev/null
        install -d /etc/apt/keyrings
        cp /site/opad-archive-keyring.asc /etc/apt/keyrings/
        printf 'Types: deb\nURIs: http://127.0.0.1:${PORT}/apt\nSuites: beta\nComponents: main\nSigned-By: /etc/apt/keyrings/opad-archive-keyring.asc\n' \
            > /etc/apt/sources.list.d/opad.sources
        apt-get update -qq
        DEBIAN_FRONTEND=noninteractive apt-get install -y -qq opad >/dev/null
        dpkg -s opad | grep -E '^(Status|Version)'
    "; then echo "✓ ${image}"; else echo "✗ ${image}"; fail=1; fi
done

for image in fedora:43 rockylinux:9; do
    echo "=== ${image}: dnf ==="
    if docker run --rm --network host "${image}" bash -euc "
        curl -fsSL http://127.0.0.1:${PORT}/rpm/opad-beta.repo -o /etc/yum.repos.d/opad.repo
        dnf -y -q install opad
        rpm -q opad
    "; then echo "✓ ${image}"; else echo "✗ ${image}"; fail=1; fi
done

echo "=== a repository signed by another key is refused ==="
if docker run --rm --network host "ubuntu:24.04" bash -euc "
    apt-get update -qq && apt-get install -y -qq gnupg >/dev/null
    gpg --batch --quiet --passphrase '' --quick-generate-key 'Someone else <x@invalid>' rsa3072 sign 1d
    install -d /etc/apt/keyrings
    gpg --armor --export > /etc/apt/keyrings/opad-archive-keyring.asc
    printf 'Types: deb
URIs: http://127.0.0.1:${PORT}/apt
Suites: beta
Components: main
Signed-By: /etc/apt/keyrings/opad-archive-keyring.asc
'         > /etc/apt/sources.list.d/opad.sources
    out=\$(apt-get update 2>&1 || true)
    echo \"\$out\" | grep -qE 'NO_PUBKEY|not signed|signatures couldn.t be verified|is not valid'
    ! apt-get install -y -qq opad >/dev/null 2>&1
"; then echo "✓ refused"; else echo "✗ accepted a foreign signature"; fail=1; fi

exit "${fail}"
