#!/usr/bin/env python3
"""
Turn an `osupadctl latency` run into the finished markdown row for
docs/latency-testing.md (P3-1, §W4-3).

The measurement itself is hardware work and cannot be automated: flash the
build for the stage, reset the statistics, tap both keys fast for two minutes,
read them back. This removes the only part of that loop that is neither of
those — copying six numbers into a table by hand and getting the column order
right.

    scripts/latency_row.py --reset                  # right before you start tapping
    ... tap both keys, > 15 presses/s, for 2 minutes ...
    scripts/latency_row.py --stage "C (10 Hz)"      # prints the row; paste it

The firmware commit column is filled from git, and marked `-dirty` when the
tree has uncommitted firmware changes — a row naming a commit that is not what
was flashed is worse than a row with no commit at all.
"""

import argparse
import datetime
import re
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

# The stage names already in the tables, so a typo does not create a new row
KNOWN_STAGES = [
    "A",
    "B",
    "C (2 Hz)",
    "C (10 Hz)",
    "C (30 Hz)",
    "D",
]

FIELDS = {
    "samples": r"^Samples:\s+(\d+)",
    "p50_us": r"^p50:\s+(\d+)",
    "p99_us": r"^p99:\s+(\d+)",
    "p999_us": r"^p99\.9:\s+(\d+)",
    "max_us": r"^max:\s+(\d+)",
    "deferred": r"^Deferred reports:\s+(\d+)",
}


class NoData(Exception):
    """`osupadctl latency` ran, but the pad had nothing to report."""


def parse_latency(text):
    """The six numbers from `osupadctl latency`, as ints."""
    if "No latency data yet" in text:
        raise NoData(
            "The pad reported no samples. Connect it, reset the statistics with "
            "--reset, then tap both keys before reading them back."
        )

    out = {}
    missing = []
    for name, pattern in FIELDS.items():
        match = re.search(pattern, text, re.MULTILINE)
        if match is None:
            missing.append(name)
        else:
            out[name] = int(match.group(1))
    if missing:
        raise NoData(
            "Could not read %s from osupadctl's output. Has its wording changed?"
            % ", ".join(missing)
        )
    # A freshly reset pad answers with six zeros. That is a real answer to a
    # different question, and pasting it into the table would record a stage as
    # measured when nobody pressed a key.
    if out["samples"] == 0:
        raise NoData(
            "The pad has 0 samples, so nothing was measured. Tap both keys "
            "alternately at > 15 presses/s for 2 minutes, then read them back."
        )
    return out


def firmware_commit():
    """The short hash, with `-dirty` when firmware/ has uncommitted changes."""
    if shutil.which("git") is None:
        return "unknown"
    try:
        rev = subprocess.run(
            ["git", "-C", str(REPO_ROOT), "rev-parse", "--short", "HEAD"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        dirty = subprocess.run(
            ["git", "-C", str(REPO_ROOT), "status", "--porcelain", "firmware", "protocol"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    except (subprocess.CalledProcessError, OSError):
        return "unknown"
    return rev + "-dirty" if dirty else rev


def format_row(stats, stage, commit, date, stuck, notes):
    """One markdown row, in docs/latency-testing.md's column order."""
    cells = [
        date,
        f"`{commit}`",
        stage,
        str(stats["samples"]),
        str(stats["p50_us"]),
        str(stats["p99_us"]),
        str(stats["p999_us"]),
        str(stats["max_us"]),
        str(stats["deferred"]),
        str(stuck),
        notes,
    ]
    return "| " + " | ".join(cells) + " |"


def run_osupadctl(args, binary):
    try:
        done = subprocess.run(
            [binary, *args], capture_output=True, text=True, check=False
        )
    except FileNotFoundError:
        sys.exit(
            f"{binary} not found. Build it with `cargo build --release` in desktop/, "
            "or pass --osupadctl."
        )
    if done.returncode != 0:
        sys.exit(f"{binary} {' '.join(args)} failed:\n{done.stderr.strip()}")
    return done.stdout


def self_test():
    sample = """=== Key edge -> HID submit latency (device-side) ===
Samples:          148213
p50:              30 µs
p99:              50 µs
p99.9:            60 µs
max:              412 µs
Deferred reports: 17 (waited for the next USB poll, then sent)
"""
    stats = parse_latency(sample)
    assert stats == {
        "samples": 148213,
        "p50_us": 30,
        "p99_us": 50,
        "p999_us": 60,
        "max_us": 412,
        "deferred": 17,
    }, stats

    row = format_row(stats, "C (10 Hz)", "abc1234", "2026-09-17", 0, "")
    # Eleven columns, in the order docs/latency-testing.md declares them
    assert row.count("|") == 12, row
    assert row.startswith("| 2026-09-17 | `abc1234` | C (10 Hz) | 148213 | 30 | 50 | 60 | 412 | 17 | 0 |"), row

    # p99.9 must not be read by the p99 pattern
    assert stats["p99_us"] != stats["p999_us"]

    empty_run = sample.replace("Samples:          148213", "Samples:          0")
    for text in (
        "No latency data yet (device not connected, or old firmware)",
        "",
        empty_run,
    ):
        try:
            parse_latency(text)
        except NoData:
            pass
        else:
            raise AssertionError("missing data must not silently become a row")

    print("self-test passed")


def main():
    parser = argparse.ArgumentParser(
        description="Print the docs/latency-testing.md row for one benchmark stage.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "--stage",
        help="Which benchmark stage this run is. One of: " + ", ".join(KNOWN_STAGES),
    )
    parser.add_argument(
        "--reset",
        action="store_true",
        help="Clear the pad's samples and stop. Run this immediately before tapping.",
    )
    parser.add_argument(
        "--stuck",
        default="0",
        help="Stuck or missed keys observed during the run (default: 0)",
    )
    parser.add_argument("--notes", default="", help="The Notes column")
    parser.add_argument(
        "--date", default=None, help="Override the date (default: today, ISO)"
    )
    parser.add_argument(
        "--osupadctl", default="osupadctl", help="Path to the osupadctl binary"
    )
    parser.add_argument(
        "--input",
        type=Path,
        help="Parse saved `osupadctl latency` output instead of running it",
    )
    parser.add_argument(
        "--self-test", action="store_true", help="Check the parser and exit"
    )
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    if args.reset:
        run_osupadctl(["latency", "--reset"], args.osupadctl)
        print("Samples cleared. Tap both keys alternately at > 15 presses/s for 2")
        print("minutes, then re-run with --stage to get the row.")
        return

    if not args.stage:
        parser.error("--stage is required (or use --reset / --self-test)")
    if args.stage not in KNOWN_STAGES:
        print(
            f"warning: '{args.stage}' is not one of the stages already in the table "
            f"({', '.join(KNOWN_STAGES)}); the row will not line up with an existing one.",
            file=sys.stderr,
        )

    text = (
        args.input.read_text()
        if args.input
        else run_osupadctl(["latency"], args.osupadctl)
    )
    try:
        stats = parse_latency(text)
    except NoData as e:
        sys.exit(str(e))

    date = args.date or datetime.date.today().isoformat()
    print(format_row(stats, args.stage, firmware_commit(), date, args.stuck, args.notes))


if __name__ == "__main__":
    main()
