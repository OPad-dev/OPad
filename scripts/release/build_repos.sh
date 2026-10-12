#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# Builds the signed apt and dnf repositories as static files
# (docs/specs/release-and-updates.md §10.1).
#
#   build_repos.sh OUT CHANNEL=DIR [CHANNEL=DIR ...]
#
#   OUT      the site root (the repositories go in OUT/apt and OUT/rpm, the
#            public key in OUT/opad-archive-keyring.asc)
#   CHANNEL  stable or beta; DIR holds that channel's .deb and .rpm files
#
# Signs with the GPG key GPG_KEY_ID from the current keyring; GPG_PASSPHRASE
# is used when set. update-feed.yml runs this with the release key (§19.3),
# scripts/release/test_repos.sh with a throwaway one.
# Needs: apt-ftparchive (apt-utils), createrepo_c, gpg.
#
# Users then add (Debian/Ubuntu, /etc/apt/sources.list.d/opad.sources):
#   Types: deb
#   URIs: <site>/apt
#   Suites: stable
#   Components: main
#   Signed-By: /etc/apt/keyrings/opad-archive-keyring.asc
# or (Fedora/Nobara/RHEL, /etc/yum.repos.d/opad.repo) the opad-<channel>.repo
# written below.
# -----------------------------------------------------------------------------

OUT="$1"; shift
: "${GPG_KEY_ID:?set GPG_KEY_ID to the signing key}"
BASE_URL="${BASE_URL:-https://opad-dev.github.io/OPad}"

for tool in apt-ftparchive createrepo_c gpg; do
    command -v "$tool" >/dev/null || { echo "ERROR: $tool is required" >&2; exit 1; }
done

gpg_sign() {
    local args=(--batch --yes --pinentry-mode loopback --local-user "${GPG_KEY_ID}")
    if [ -n "${GPG_PASSPHRASE:-}" ]; then
        gpg "${args[@]}" --passphrase-fd 3 "$@" 3<<<"${GPG_PASSPHRASE}"
    else
        gpg "${args[@]}" "$@"
    fi
}

mkdir -p "${OUT}/apt" "${OUT}/rpm"
gpg --armor --export "${GPG_KEY_ID}" > "${OUT}/opad-archive-keyring.asc"

for spec in "$@"; do
    channel="${spec%%=*}"
    dir="${spec#*=}"
    case "${channel}" in stable|beta) ;; *) echo "ERROR: unknown channel ${channel}" >&2; exit 1 ;; esac

    # --- apt: dists/<channel>/main/binary-amd64, pool/<channel> ---
    (
        cd "${OUT}/apt"
        rm -rf "pool/${channel}" "dists/${channel}"
        mkdir -p "pool/${channel}" "dists/${channel}/main/binary-amd64"
        find "${dir}" -maxdepth 1 -name '*.deb' -exec cp {} "pool/${channel}/" \;
        apt-ftparchive packages "pool/${channel}" > "dists/${channel}/main/binary-amd64/Packages"
        gzip -9kf "dists/${channel}/main/binary-amd64/Packages"
        apt-ftparchive \
            -o APT::FTPArchive::Release::Origin=OPad \
            -o APT::FTPArchive::Release::Label=OPad \
            -o "APT::FTPArchive::Release::Suite=${channel}" \
            -o "APT::FTPArchive::Release::Codename=${channel}" \
            -o APT::FTPArchive::Release::Architectures=amd64 \
            -o APT::FTPArchive::Release::Components=main \
            release "dists/${channel}" > "dists/${channel}/Release.tmp"
        mv "dists/${channel}/Release.tmp" "dists/${channel}/Release"
        gpg_sign --clearsign -o "dists/${channel}/InRelease" "dists/${channel}/Release"
        gpg_sign --armor --detach-sign -o "dists/${channel}/Release.gpg" "dists/${channel}/Release"
    )

    # --- dnf: rpm/<channel> with signed repodata ---
    rm -rf "${OUT}/rpm/${channel}"
    mkdir -p "${OUT}/rpm/${channel}/x86_64"
    find "${dir}" -maxdepth 1 -name '*.rpm' -exec cp {} "${OUT}/rpm/${channel}/x86_64/" \;
    createrepo_c --quiet "${OUT}/rpm/${channel}"
    gpg_sign --armor --detach-sign -o "${OUT}/rpm/${channel}/repodata/repomd.xml.asc" \
        "${OUT}/rpm/${channel}/repodata/repomd.xml"
    cat > "${OUT}/rpm/opad-${channel}.repo" <<EOF
[opad]
name=OPad (${channel})
baseurl=${BASE_URL}/rpm/${channel}/
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=${BASE_URL}/opad-archive-keyring.asc
EOF
    echo "✓ ${channel}: $(ls "${OUT}/apt/pool/${channel}" | wc -l) .deb, $(ls "${OUT}/rpm/${channel}/x86_64" | wc -l) .rpm"
done
