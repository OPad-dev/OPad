You are writing the documentation of OPad, in the repository you are running
in. This is LAYER 1 of 3: you write every page. Layer 2 then checks each
claim against the code, and layer 3 finishes the critical pages. You work
for a long time, possibly over several sessions, so all your state is kept
in files (see "State and resuming").

# 0. Read these first, every session

1. `AGENTS.md`, all of it. These are the repository's rules and they override
   anything here except section 2 below. §1 (product rules) and §7
   (documentation) matter most.
2. `README.md` and `docs/README.md`.
3. `git log --oneline -150`.
4. If `docs-work/PLAN.md` exists, read it, plus `docs-work/FINDINGS.md` and
   `docs-work/GLOSSARY.md`, and go on from the first page whose status is not
   `draft`.

# 1. What OPad is (verify everything in the code, this is only orientation)

A two-key osu! keypad on the Waveshare ESP32-S3-Touch-LCD-2. Firmware is in
`firmware/` (ESP-IDF, C). The desktop suite is in `desktop/` (Rust): a daemon
(`desktop/daemon`), a GUI (`desktop/gui`), the CLI `opadctl` (`desktop/cli`)
and shared crates (`desktop/crates/*`). The wire protocol is
`protocol/opad.proto`. Hardware (KiCad, case) is in `hardware/`. Packaging is
in `packaging/`, scripts in `scripts/`, CI in `.github/workflows/`.

# 2. Hard rules (breaking one makes the page wrong)

1. **The code is the only source of truth.** The existing files in `docs/`
   are possibly stale. Use them only to find out what topics exist and which
   files to read. Never copy a fact from an old doc without finding it in
   the code.
2. **Every factual claim cites the file it came from**, a file you actually
   opened and read in this session (see §5 for the format). If you cannot
   find the source, do not write the claim. If you believe it is true but
   could not confirm it, write it followed by
   `<!-- TODO(verify): what is unsure and where you looked -->`.
3. **Never invent.** That includes command flags, config keys, file paths,
   GUI labels, error messages, numbers and default values. Copy them
   character for character from the code.
4. **Do not change anything outside `docs/` and `docs-work/`.** No code, no
   `README.md`, no `AGENTS.md`, no generated files. If you find a bug or
   something in the code that looks wrong, write it in
   `docs-work/FINDINGS.md` and keep documenting what the code does now.
5. **Never tell anyone to press the BOOT or RESET buttons**, not even as a
   fallback. Flashing and recovery happen over USB with `opadctl flash` and
   `opadctl recover` (read `desktop/cli/src/main.rs` and `docs/recovery.md`
   §7).
6. **Never say the app and the pad must be updated together.** They are
   designed to work in either version combination (AGENTS.md §1.3).
7. **Never describe or suggest macros, auto-tap, or any way to make the pad
   send a key without a finger** (AGENTS.md §1.6).
8. **No placeholder pages.** A page with nothing verified yet is not created.
   Leave it as `todo` in the plan.
9. **Never touch hardware or the running system.** Do not run `opadctl flash`,
   `opadctl recover`, `idf.py flash`, espflash, the daemon or the GUI.
   Allowed: reading files, `git log`/`git show`/`git grep`, `ls`, `cargo build`
   and `cargo run -q -p opadctl -- <subcommand> --help` inside `desktop/`.
10. **Do not edit `docs/history/`**. It is the archive.

# 3. Paths that must not move

Code and CI print or reference these paths and section numbers in
user-facing messages. Keep them exactly where they are, keep their numbered
headings (`## 7.`, `### 7.6` ...) and keep the meaning of each numbered
section:

- `docs/recovery.md` (code cites §3, §5, §7, §7.6, §7.7)
- `docs/troubleshooting.md` (code cites §2)
- `docs/testing-checklist.md`
- `docs/README.md`
- `docs/specs/release-and-updates.md`, `docs/specs/v2-hall-effect-decisions.md`
- `docs/hardware/components/ads7028.md`

Check with `git grep -n "docs/"` before moving any other file. When you move
one, update every link to it inside `docs/` and record the move in
`docs-work/PLAN.md`.

# 4. Site and structure

The docs are published by a separate Astro **Starlight** site that reads this
repo's `docs/` folder. You do not write any Astro code. What follows from it:

- **English is the root locale** (`docs/*.md`), **Spanish mirrors it** at
  `docs/es/` with the same file names and the same headings in the same
  order. More languages come later, so never hardcode "English/Spanish" in
  page text.
- Every page starts with frontmatter:
  ```yaml
  ---
  title: Recovering a pad
  description: One sentence that says what the page is for.
  sidebar:
    order: 3
  ---
  ```
  Pages that are internal (for the owner, not for the public site) add
  `draft: true`. Internal: `launch-checklist.md`, `pad-test-sheet.md`.
- Since the title is in the frontmatter, the body does not start with a `#`
  heading. It starts with text, then `##` sections. Existing pages that keep
  their path (§3) keep their numbered `##` headings.
- `docs/history/` is not published. `docs/specs/` is published as "Design
  records": add frontmatter to those files, do not rewrite them, do not
  translate them.
- The structure follows Diátaxis. Each page is exactly one of: tutorial
  (learning, step by step), how-to (one task), reference (exact facts,
  tables), explanation (why it works this way). The audience is both
  users and contributors: user pages come first and never require reading
  developer pages.

Target tree (adjust names if the code shows a better split, and write why in
the plan):

```
docs/
  index.md                      what OPad is; "use it" vs "develop it"
  start/                        TUTORIALS (users)
    what-you-need.md
    first-connection.md         plug in: it is a keyboard, no software needed
    install-the-app.md          Linux packages and Windows installer
    wiring-the-keys.md
  guides/                       HOW-TO (users)
    update-firmware.md
    change-key-pins.md
    touchscreen-and-swipes.md
    calibrate-hall-effect.md
    use-with-tosu.md
    backup-and-restore.md
    uninstall.md
  recovery.md                   (fixed path, see §3)
  troubleshooting.md            (fixed path, see §3)
  reference/                    REFERENCE
    opadctl.md                  from `--help` of every subcommand, verbatim
    gui.md                      every page and setting of the GUI
    device-settings.md          every pad setting, type, range, default
    pinout.md                   header pins, defaults, which pins are free
    serial-protocol.md          framing, messages, fields (from opad.proto)
    ipc.md                      daemon <-> GUI/CLI
    files-and-paths.md          where config, database, logs live per OS
  explanation/                  EXPLANATION
    architecture.md
    latency.md
    touchscreen-third-button.md
    app-and-pad-versions.md     independent updates, framing negotiation
    counters-and-ownership.md
    updates-and-rollback.md     OTA slots, rollback, update feed
    hall-effect.md
  develop/                      CONTRIBUTORS (all four types allowed)
    index.md                    workflow and rules, linking AGENTS.md
    building.md
    testing.md                  `make check`, host tests, links testing-checklist.md
    firmware/  overview.md input.md usb.md touch.md ui.md config-and-nvs.md
               counters.md diag.md
    desktop/   overview.md daemon.md gui.md opadctl.md crates.md
    protocol-changes.md         editing opad.proto, gen_proto.sh, compatibility
    hardware.md                 hardware/ folder, KiCad, case
    packaging.md
    releases.md
    lab.md                      the CI lab (scripts/lab, lab-*.yml workflows)
    latency-testing.md
  specs/                        design records (frontmatter only)
  hardware/components/          datasheet notes (frontmatter only)
  es/                           Spanish mirror of everything above except
                                specs/ and hardware/components/
```

The old top-level pages (`architecture.md`, `protocol.md`,
`windows-portability.md`, `latency-testing.md`, `roadmap.md`) are folded into
the new pages. Once everything true in one of them has a place in the new
tree, delete it (`git rm`), fix the links, and record that in the plan. Never
delete a page whose content you have not moved or found to be false. Anything
false goes in FINDINGS ("doc said X, code says Y").

# 5. How to cite

- **User pages** (`start/`, `guides/`, `reference/`, `recovery.md`,
  `troubleshooting.md`): keep the text clean. After each paragraph, list
  item, or table row that states a fact, add an invisible comment:
  `<!-- src: /desktop/cli/src/main.rs#fn_name, /firmware/main/usb/usb_cdc.c -->`.
  Use a path from the repo root starting with `/`. `#symbol` is optional; it
  is a function, constant, struct or message name that literally appears in
  that file.
- **Explanation and develop pages**: link the sources visibly, for example
  ``[`reboot_to_rom_download()`](/firmware/main/usb/usb_cdc.c)``. Use
  repo-root links starting with `/`, never `../../`. The `src` comments are
  also fine here.
- **Links between docs pages**: relative and with `.md`, for example
  `[recovery](../recovery.md#7-reflashing--unbinding-a-pad-w3-4)`.
- **No line numbers** (`#L120`). They go stale.

# 6. Writing style

- Plain, short sentences, second person for users ("Plug the pad in").
  No marketing language, no emojis, no "simply" or "just".
- Say what happens, not what the code is named, on user pages. On develop
  pages, name the files and functions.
- Tables for reference facts. Fenced code blocks with a language for
  commands (`sh`, `powershell`, `toml`, `c`, `rust`).
- Diagrams in Mermaid (```mermaid) only where they help.
- Every user how-to has, in order: what you need, steps, how to check it
  worked, and what to do if it did not (a link to troubleshooting or
  recovery).

# 7. State and resuming

Create `docs-work/` at the repo root with these files and keep them current:

- `docs-work/PLAN.md`: one table row per page:
  `| path | type | status | sources read | notes |`. Status is one of
  `todo`, `draft`, `translated`. Layers 2 and 3 add later statuses; never
  set those. Fill the "sources" column before writing the page: run `ls`
  and `git grep` to find every file that implements the topic.
- `docs-work/FINDINGS.md`: numbered entries `F1`, `F2` ... Each one has the
  page, the problem (code bug, doc/code conflict, missing feature, unclear
  behaviour) and the files involved.
- `docs-work/GLOSSARY.md`: term table `| English | Español | note |`. Keep
  product and game terms in English in both languages: OPad, osu!, tosu,
  Quick Retry, Rapid Trigger, Hall Effect, HID, OTA, `opadctl`, map,
  daemon. Add every term you translate so the translation stays consistent.
- `docs-work/check_docs.py` is already there; do not edit it. Run
  `python3 docs-work/check_docs.py` from the repo root after every page.
  Errors must be zero before you commit the page. The warnings for pages you
  have not reached yet are expected.
- `docs-work/prompts/` holds the prompts of the three layers. Do not edit
  them.

# 8. Order of work

Work only on the branch `docs-wiki` (`git branch --show-current` must print
it). If it prints anything else, stop and tell the user. Never commit to
`main`, never merge, rebase, push or delete branches. Commit after every finished page, with the
user's git identity (never set yourself as author), in this format:

```
docs(<area>): <what the page now documents>

AI: generated
```

Phases:

1. **Plan.** Read the whole repo tree (`git ls-files`), write `PLAN.md` with
   every page and its sources, create `GLOSSARY.md` and `FINDINGS.md`,
   commit.
2. **Frontmatter for fixed and record pages.** Add frontmatter to
   `recovery.md`, `troubleshooting.md`, `testing-checklist.md`, `specs/*`,
   `hardware/components/*`, `launch-checklist.md`, `pad-test-sheet.md`.
   Remove the leading `#` title from each because it is now the frontmatter
   title. Do not change anything else yet. Commit.
3. **Reference pages first**, since the others link to them: `opadctl.md`
   (run `--help` for the command and every subcommand, paste the output
   verbatim, then explain each one), `device-settings.md`, `pinout.md`,
   `serial-protocol.md`, `ipc.md`, `files-and-paths.md`, `gui.md`.
4. **Explanation pages.**
5. **User tutorials and how-tos**, then rewrite `recovery.md` and
   `troubleshooting.md` against the code, keeping §3's numbering.
6. **Develop pages.**
7. **Fold and delete old pages** (§4), rewrite `docs/README.md` as a short
   contributor map of `docs/` (where things go, the rules of §2 and §5).
8. **Spanish.** Translate every page except `specs/` and
   `hardware/components/` into `docs/es/` with the same path, headings in the
   same order, and the same `src` comments. Translate the text, not the code,
   commands, flags, GUI labels or error messages (those stay exactly as the
   program prints them). Neutral Spanish, `tú` form. Relative links point to
   the Spanish pages. Mark each page `translated` in the plan.
9. **Final check.** `check_docs.py` must report 0 errors. List the warnings
   you did not fix and why in `PLAN.md`. Then stop and summarise for the
   user: pages written, pages deleted, open `TODO(verify)` count, and the
   FINDINGS list.

For each page:

1. Read every file in its sources column completely (not only the matches of
   a grep).
2. Write the page.
3. Re-read the page and, for each claim, ask yourself "which line of which
   file says this?". Remove or mark anything you cannot point to.
4. Run `check_docs.py`, fix errors, update `PLAN.md`, commit.

If the context gets long or you are unsure what you already did, stop
reading and trust `PLAN.md` and `git log`, not your memory.
