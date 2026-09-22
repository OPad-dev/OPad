#!/bin/bash
# Builds @tosuapp/lazer-calculator's Linux binding.node inside almalinux:8
# (glibc 2.28), so the copy bundled in tosu runs on every supported distro;
# upstream builds it on ubuntu-latest, which stamps glibc 2.38 on it.
# The checkout (at the version tosu pins) is mounted at /src.
set -euo pipefail
dnf install -y -q git clang zlib-devel libicu krb5-libs openssl-libs findutils tar gzip which >/dev/null
curl -fsSL https://dot.net/v1/dotnet-install.sh -o /tmp/dotnet-install.sh
bash /tmp/dotnet-install.sh --version "${DOTNET_SDK_VERSION:-10.0.401}" --install-dir /opt/dotnet >/dev/null
export PATH=/opt/dotnet:$PATH DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1
echo "dotnet $(dotnet --version)"
cd /src/lib
rev=$(sed -n 's/.*"revision": "\([0-9a-f]*\)".*/\1/p' package.json)
rm -rf vendor native/dist native/obj native/bin && mkdir vendor
git -C vendor init -q
git -C vendor remote add origin https://github.com/ppy/osu
git -C vendor fetch -q --depth 1 origin "$rev"
git -C vendor checkout -q FETCH_HEAD
git -C vendor apply --whitespace=fix -C0 ../patches/0001-Gradual-diff-calculator.patch
# binding.csproj takes ppy.osu.Game.Resources at version "*": whatever is
# newest the day it builds. Use the version the pinned osu! revision names.
res=$(grep -o '"ppy.osu.Game.Resources" Version="[^"]*"' vendor/osu.Game/osu.Game.csproj | grep -o '[0-9][0-9.]*')
sed -i "s/\"ppy.osu.Game.Resources\" Version=\"\*\"/\"ppy.osu.Game.Resources\" Version=\"$res\"/" native/binding.csproj
grep -q "Version=\"$res\"" native/binding.csproj
echo "osu! $rev, ppy.osu.Game.Resources $res"
dotnet publish native --ucr 2>&1 | grep -E "error|binding ->" || true
test -f native/dist/binding.node
# Hand the tree back to whoever owns the checkout (the container runs as root)
chown -R "$(stat -c %u:%g /src)" /src
