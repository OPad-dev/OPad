#!/usr/bin/env python3
"""
Generate an osu!standard test map for the tap rate (PPM) statistics, issue #2.

The map is light, then an ultra burst, then light again, at one BPM, with no
music (a silent track keeps time). Every note is one press, so what the pad
and the Stats page should read is known in advance; the script prints it.

    light   1/1 jumps  x24   -> live 1 x BPM
            1/2 jumps  x32   -> live 2 x BPM
    burst   1/4 stream, at least 10.5 s  -> PEAK and Best 10 s: 4 x BPM
    light   1/2 jumps  x16, then 1/1 x24

Usage:
    python3 scripts/ppm_test_map.py [--bpm 180] [--out FILE.osz]

Needs ffmpeg for the silent audio. Open the .osz with osu! to import it, then
follow PPM-01 in docs/testing-checklist.md.
"""

import argparse
import math
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile

STREAM_MIN_S = 10.5  # longer than the 10 s window, so Best 10 s is the stream


def build_notes(bpm):
    """(time_ms, x, y, new_combo) for every note, and the stream's length"""
    beat = 60000.0 / bpm
    offset = 2000.0
    notes = []

    def light(start_beat, count, step_beats):
        # Wide back-and-forth jumps around the playfield centre
        corners = [(156, 112), (356, 112), (356, 272), (156, 272)]
        for i in range(count):
            x, y = corners[i % 4]
            notes.append((offset + (start_beat + i * step_beats) * beat, x, y, i % 4 == 0))
        return start_beat + count * step_beats

    def stream(start_beat, count):
        # 1/4 notes on a slowly turning circle, ~22 px apart
        cx, cy, r = 256, 192, 120
        for i in range(count):
            a = i * 22.0 / r
            notes.append((offset + (start_beat + i * 0.25) * beat,
                          cx + r * math.cos(a), cy + r * math.sin(a), i % 16 == 0))
        return start_beat + count * 0.25

    stream_notes = 16 * math.ceil(STREAM_MIN_S * 1000 / (beat / 4) / 16)
    b = light(0, 24, 1.0)
    b = light(b, 32, 0.5)
    b = stream(b + 1, stream_notes)
    b = light(b + 1, 16, 0.5)
    light(b, 24, 1.0)
    return notes, beat, offset, stream_notes


def osu_file(notes, bpm, beat, offset, version):
    head = f"""osu file format v14

[General]
AudioFilename: silence.mp3
AudioLeadIn: 0
PreviewTime: {int(offset)}
Countdown: 0
SampleSet: Soft
StackLeniency: 0.7
Mode: 0
LetterboxInBreaks: 0
WidescreenStoryboard: 0

[Editor]
DistanceSpacing: 1
BeatDivisor: 4
GridSize: 16
TimelineZoom: 1

[Metadata]
Title:OPad PPM Test
TitleUnicode:OPad PPM Test
Artist:OPad
ArtistUnicode:OPad
Creator:OPad
Version:{version}
Source:
Tags:opad ppm test burst stream
BeatmapID:0
BeatmapSetID:-1

[Difficulty]
HPDrainRate:2
CircleSize:4
OverallDifficulty:7
ApproachRate:9
SliderMultiplier:1.4
SliderTickRate:1

[Events]
//Background and Video events
//Break Periods

[TimingPoints]
{int(offset)},{beat},4,2,0,50,1,0

[HitObjects]
"""
    return head + "\n".join(
        f"{round(x)},{round(y)},{round(t)},{5 if nc else 1},0,0:0:0:0:" for t, x, y, nc in notes
    ) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--bpm", type=float, default=180.0, help="map BPM (default 180)")
    parser.add_argument("--out", help="output .osz (default: in the current directory)")
    args = parser.parse_args()
    if not 60 <= args.bpm <= 400:
        sys.exit("--bpm must be between 60 and 400")
    if shutil.which("ffmpeg") is None:
        sys.exit("ffmpeg is needed for the silent audio track")

    bpm = args.bpm
    version = f"Light - Ultra Burst - Light ({bpm:g} BPM)"
    notes, beat, offset, stream_notes = build_notes(bpm)
    out = args.out or f"OPad PPM Test - {version}.osz"

    with tempfile.TemporaryDirectory() as tmp:
        audio = os.path.join(tmp, "silence.mp3")
        audio_s = math.ceil(notes[-1][0] / 1000.0) + 4
        subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-f", "lavfi",
                        "-i", "anullsrc=r=44100:cl=stereo", "-t", str(audio_s),
                        "-q:a", "9", audio], check=True)
        with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
            z.writestr(f"OPad - OPad PPM Test (OPad) [{version}].osu",
                       osu_file(notes, bpm, beat, offset, version))
            z.write(audio, "silence.mp3")

    n = len(notes)
    length_s = (notes[-1][0] - notes[0][0]) / 1000.0
    print(f"Written: {out}")
    print(f"{n} notes over {length_s:.1f} s of song (first to last note), "
          f"burst: {stream_notes} notes of 1/4")
    print("Expected, hitting every note once (allow about +-10%):")
    print(f"  AVG PPM at the end    {n / length_s * 60:.0f}")
    print(f"  PEAK                  {4 * bpm:.0f}  (a rushed burst reads a little higher)")
    print(f"  Best 10 s             {4 * bpm:.0f}")
    print(f"  Live, light parts     {bpm:.0f} on 1/1, {2 * bpm:.0f} on 1/2")
    print(f"  K1 / K2 in the burst  {2 * bpm:.0f} each when alternating")


if __name__ == "__main__":
    main()
