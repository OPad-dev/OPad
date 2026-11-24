#!/usr/bin/env bash
# The ci-linux VM's half of scripts/ci/lab-ci.sh: checks the commit out of
# the bundle lab-ci.sh copied in, brings the VM's tools up to date
# (provision-linux.sh) and runs every Linux job of CI (jobs.sh), each to the
# end even after another failed. Prints "JOBS <names>", then one
# "RESULT <job> pass|fail <time>" line per job; exits 1 if any failed.
#
#   lab-run.sh <bundle> <sha>
set -uo pipefail

bundle="$1" sha="$2"
work="$HOME/ci/repo"

set -e
[ -d "$work/.git" ] || git init -q "$work"
cd "$work"
git fetch -q "$bundle" "refs/lab-ci/$sha"
git checkout -q -f --detach "$sha"
# A clean checkout, keeping only the caches
git clean -q -ffdx -e /desktop/target/ -e /.ccache/ -e /firmware/managed_components/
bash scripts/ci/provision-linux.sh
set +e

# The VM's disk image only grows, on a host with little room: a build
# cache past 10 GB starts over (it rebuilds in a few minutes)
if [ -d desktop/target ] && [ "$(du -sx --block-size=1G desktop/target | cut -f1)" -gt 10 ]; then
    echo "desktop/target is over 10 GB: removing it"
    rm -rf desktop/target
fi

# shellcheck disable=SC1091
. "$HOME/.cargo/env"
export PATH="$HOME/ci-venv/bin:$HOME/.local/bin:$PATH"
export CARGO_TERM_COLOR=never
# A smaller build cache on a small disk: no incremental state and no debug
# info, neither of which changes what the jobs check
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0

jobs="$(scripts/ci/jobs.sh list)"
echo "JOBS $jobs"
failed=0
for job in $jobs; do
    start=$SECONDS
    if scripts/ci/jobs.sh "$job"; then r=pass; else r=fail; failed=1; fi
    echo "RESULT $job $r $((SECONDS - start))s"
done
# Hands the space freed during the run back to the mini PC: ci-linux's disk
# has discard on, so trimmed blocks leave its image (lab.md)
sudo -n fstrim / >/dev/null 2>&1 || true
exit "$failed"
