#!/bin/bash
# compare.sh UPSTREAM_BINDING OUR_BINDING BEATMAP_DIR
#
# Checks a rebuilt lazer-calculator against upstream's prebuilt: star rating
# and pp at 97% for every .osu in BEATMAP_DIR (the osu! repo's
# osu.Game.Tests/Resources works), with no mods, DT and HR+HD. One process per
# beatmap and build: two NativeAOT runtimes in one process hang, and some test
# maps take minutes, so each gets 30 s. Prints the first differences, if any.
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
A=$1; B=$2; D=$3
OUT=$(mktemp -d)
for path in "$D"/*.osu; do
  f=$(basename "$path")
  for side in a b; do
    lib=$A; [ $side = b ] && lib=$B
    timeout 30 node "$HERE/calc.cjs" "$lib" "$D" "$f" >> "$OUT/$side.jsonl" 2>/dev/null \
      || echo "{\"f\":\"$f\",\"timeout_or_crash\":true}" >> "$OUT/$side.jsonl"
  done
done
if cmp -s "$OUT/a.jsonl" "$OUT/b.jsonl"; then
  echo "IDENTICAL: $(grep -c '"stars"' "$OUT/b.jsonl") results with stars and pp ($OUT)"
else
  echo "DIFFERENT ($OUT):"; diff "$OUT/a.jsonl" "$OUT/b.jsonl" | head -20; exit 1
fi
