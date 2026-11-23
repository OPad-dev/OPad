#!/usr/bin/env bash
# The Linux jobs of CI, one function each. .github/workflows/ci.yml runs
# them on GitHub after installing the tools, and scripts/ci/lab-ci.sh runs
# the same ones in the lab's ci-linux VM (scripts/ci/provision-linux.sh), so
# a check passes or fails the same way in both places.
#
#   scripts/ci/jobs.sh <job>...    jobs: versions rust firmware-host-tests
#                                  proto-check firmware-build licences
#   scripts/ci/jobs.sh all         every job above, in that order
#   scripts/ci/jobs.sh list        their names
#
# Run from anywhere; it works from the repository root.
set -euo pipefail

SELF="$(realpath "${BASH_SOURCE[0]}")"
cd "$(dirname "$SELF")/../.."

LINUX_JOBS=(versions rust firmware-host-tests proto-check firmware-build licences)
IDF_IMAGE="espressif/idf:v5.5.2"

job_versions() {
    # SemVer in desktop/Cargo.toml and firmware/CMakeLists.txt, the same in
    # .release-please-manifest.json and Cargo.lock
    python3 scripts/release/check_versions.py
}

job_rust() {
    (
        cd desktop
        cargo fmt --check
        cargo clippy --workspace --all-targets -- -D warnings
        cargo test --workspace
    )
}

job_firmware-host-tests() {
    ./firmware/test/host/run_tests.sh
}

job_proto-check() {
    # The committed opad.pb.[ch] name the generator version: nanopb 0.4.9.1
    ./scripts/gen_proto.sh
    git diff --exit-code firmware/main/protocol/
}

job_firmware-build() {
    if [ -f /opt/esp/idf/export.sh ]; then
        # Inside espressif/idf (GitHub's job container, or the docker run below)
        # shellcheck disable=SC1091
        . /opt/esp/idf/export.sh
        export CCACHE_DIR="${CCACHE_DIR:-$PWD/.ccache}"
        if command -v ccache >/dev/null; then export IDF_CCACHE_ENABLE=1; fi
        cd firmware
        idf.py set-target esp32s3 build
        if command -v ccache >/dev/null; then ccache --show-stats; fi
    else
        # The lab: the same image GitHub uses, as this user so that nothing
        # in the checkout ends up owned by root
        docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp \
            -v "$PWD:/w" -w /w "$IDF_IMAGE" \
            bash -c 'git config --global --add safe.directory "*" && scripts/ci/jobs.sh firmware-build'
    fi
}

job_licences() {
    # Plain `reuse lint` can never pass: the easter-egg GIF is deliberately
    # kept under LicenseRef-Unknown-ThirdParty. This fails on anything else.
    python3 scripts/check_reuse.py
    # As EmbarkStudios/cargo-deny-action ran it (its default arguments)
    cargo deny --manifest-path desktop/Cargo.toml --all-features check
    # cargo about runs --offline, reading licence files from the crates
    (cd desktop && cargo fetch --locked)
    make notices
}

if [ $# -eq 0 ]; then
    sed -n '2,13p' "$SELF" | sed 's/^# \{0,1\}//'
    exit 2
fi
if [ "$1" = list ]; then
    echo "${LINUX_JOBS[*]}"
    exit 0
fi
if [ "$1" = all ]; then
    set -- "${LINUX_JOBS[@]}"
fi
for job in "$@"; do
    if ! declare -F "job_$job" >/dev/null; then
        echo "unknown job: $job (jobs: ${LINUX_JOBS[*]})" >&2
        exit 2
    fi
    echo "::group::$job"
    "job_$job"
    echo "::endgroup::"
done
