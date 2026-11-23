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

# shellcheck disable=SC1091
. "$HOME/.cargo/env"
export PATH="$HOME/ci-venv/bin:$HOME/.local/bin:$PATH"
export CARGO_TERM_COLOR=never

jobs="$(scripts/ci/jobs.sh list)"
echo "JOBS $jobs"
failed=0
for job in $jobs; do
    start=$SECONDS
    if scripts/ci/jobs.sh "$job"; then r=pass; else r=fail; failed=1; fi
    echo "RESULT $job $r $((SECONDS - start))s"
done
exit "$failed"
