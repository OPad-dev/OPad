#!/usr/bin/env bash
# Sets up the lab's ci-linux VM (Ubuntu 24.04, a full clone of the
# ubuntu-24.04 VM's "limpia" snapshot, kept between runs so the caches
# stay) with what scripts/ci/jobs.sh needs: the same apt packages as
# .github/workflows/ci.yml, Rust stable, cargo-deny and cargo-about, nanopb
# 0.4.9.1, reuse, and Docker with the espressif/idf:v5.5.2 image. Safe to
# run again; scripts/ci/lab-ci.sh does on every run, so a change here
# reaches the VM by itself.
set -euo pipefail

stamp="$HOME/.ci-provisioned-$(sha256sum "$0" | cut -c1-16)"
[ -f "$stamp" ] && exit 0

export DEBIAN_FRONTEND=noninteractive
sudo hostnamectl set-hostname ci-linux

sudo apt-get update
sudo apt-get install -y --no-install-recommends \
    build-essential pkg-config git curl ca-certificates make \
    libasound2-dev libudev-dev libx11-dev libxkbcommon-dev libwayland-dev \
    libfontconfig1-dev libdbus-1-dev \
    python3 python3-venv pipx cloud-guest-utils docker.io
sudo usermod -aG docker "$USER"

# The clone's disk was grown to 80 GB; take the space
if command -v growpart >/dev/null && sudo growpart --dry-run /dev/sda 2 >/dev/null 2>&1; then
    sudo growpart /dev/sda 2 && sudo resize2fs /dev/sda2
fi

if ! command -v rustup >/dev/null && [ ! -x "$HOME/.cargo/bin/rustup" ]; then
    curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
# shellcheck disable=SC1091
. "$HOME/.cargo/env"
rustup toolchain install stable --profile minimal -c rustfmt,clippy
rustup default stable
cargo install --locked cargo-deny
# Its binary is behind a feature
cargo install --locked cargo-about --features cli

# nanopb pinned: the committed opad.pb.[ch] name the generator version
python3 -m venv "$HOME/ci-venv"
"$HOME/ci-venv/bin/pip" install --upgrade pip
"$HOME/ci-venv/bin/pip" install protobuf grpcio-tools nanopb==0.4.9.1
pipx install reuse

sudo docker pull espressif/idf:v5.5.2

touch "$stamp"
echo "ci-linux provisioned"
