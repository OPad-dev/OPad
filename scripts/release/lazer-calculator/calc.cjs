// Star rating and pp for every beatmap in a directory, with a few mod sets,
// from one build of the lazer calculator; one JSON line per run.
// One build per process: two NativeAOT runtimes in one process hang.
const fs = require('fs'), path = require('path');
const [lib, dir, only] = process.argv.slice(2);
const L = require(lib);
for (const f of fs.readdirSync(dir).filter(f => f.endsWith('.osu') && (!only || f === only)).sort()) {
  const content = fs.readFileSync(path.join(dir, f), 'utf8');
  for (const mods of [[], ['DT'], ['HR', 'HD']]) {
    let out;
    try {
      const bm = L.PlayBeatmap.parse(content);
      bm.applyMods(mods.map(acronym => ({ acronym, settings: new Map() })));
      const gd = bm.createGradualDifficulty();
      gd.skipToEnd();
      const attrs = gd.createDifficultyAttrs();
      const perf = bm.calculatePerformance(attrs, bm.createScore(0.97));
      out = { mode: bm.mode, stars: attrs.getData().stars, pp: perf.pp };
    } catch (e) {
      out = { error: String(e.message || e).split('\n')[0].slice(0, 80) };
    }
    console.log(JSON.stringify({ f, mods, ...out }));
  }
}
