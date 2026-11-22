You are LAYER 3 of 3, the final pass on the OPad documentation. Layer 1
(a cheap model) wrote it and layer 2 checked each claim against the code. You
are the most capable model in the chain. Your job: make it correct where it
matters most, coherent as a whole, and ready to publish. Then hand it to the
owner for review. You do not merge it.

# 0. Context

1. Read `AGENTS.md` entirely.
2. Read `docs-work/prompts/1-draft.md` (format and rules) and
   `docs-work/prompts/2-verify.md`.
3. Read `docs-work/PLAN.md` (including the "Layer 2 report"),
   `docs-work/FINDINGS.md` and `docs-work/GLOSSARY.md`.
4. `git branch --show-current` must print `docs-wiki`.
   `git log --oneline main..docs-wiki` shows what layers 1 and 2 did. Never
   commit to `main`, merge, rebase, push or delete branches unless the owner
   asks.
5. Run `python3 docs-work/check_docs.py`.

**Do not stop until the whole task is done** (§4 complete). Do not stop to
report progress or wait for an OK. Anything §3 forbids you to decide goes in
the final report, and you keep going with the rest.

## 0.1 The code moved after layer 2. Re-verify that first.

Layers 1 and 2 checked the docs against commit `86f7bf0`. The owner rebases
`docs-wiki` onto `main` before you start, so the code you see now is newer.

1. Run `python3 docs-work/affected_pages.py 86f7bf0`. It lists every page
   (en and es) that cites a file changed since then.
2. For each listed page, read `git diff 86f7bf0 HEAD -- <the changed files>`
   and fix every claim the change makes wrong. Known ones: `opadctl update`
   is new (`desktop/cli/src/main.rs`, `reference/opadctl.md`); the screen's
   memory moved to PSRAM, with 40-line draw buffers and a 30 fps cap
   (`firmware/main/ui/ui_port.c`, `sdkconfig.defaults`,
   `develop/firmware/ui.md`, `explanation/architecture.md`); and the update
   policy changed (silent app updates, firmware installed by itself after
   30 min unused, "Update now" still asks, never during a map). Document the
   update policy only as far as it is in the code on this branch.
3. Pages that changes add a topic to without citing it yet: run
   `git log --oneline 86f7bf0..HEAD -- ':!docs'` and check each `feat` and
   `fix` commit for user-visible behaviour that no page covers.

## 0.2 Items layer 2 left for you

- FINDINGS F3 (host-driven updates need the pad running from `ota_0`,
  `Blocker::NeedsFullReflash` in `desktop/crates/opad-update/src/firmware.rs`)
  and F7 (the app's `KEY_PINS` in `desktop/crates/opad-model/src/lib.rs`
  offers GPIO 8, which `KEY_GPIO_ALLOWED` in
  `firmware/main/config/config_validate.c` rejects): confirm each in the code
  and turn it into a question for the owner with a recommendation (§3). Do
  not change code. The docs describe what the code does now.
- Write `docs/guides/customize-screen-layout.md` (en and es): a how-to for
  the Screen Designer (building, exporting and importing Idle/Playing
  layouts), from `desktop/gui/src/designer/` and the firmware's `ui/` code,
  listing every selectable data source (`ui_source_t`).
- `docs/es/testing-checklist.md`: translate the headings that are still in
  English, keeping the heading count the same as the English page.

The site is a separate Astro Starlight repo that reads `docs/`: English at
the root, Spanish in `docs/es/`, `draft: true` hides a page, and
`docs/history/` is not published. Links to source files are repo-root
(`/firmware/...`) and get rewritten to GitHub URLs by a remark plugin on the
site side.

# 1. Critical pages: re-verify them yourself

Do not trust layer 2 on these. Read the code paths end to end and check every
step a user would follow:

- `docs/recovery.md`, `docs/guides/update-firmware.md`,
  `docs/explanation/updates-and-rollback.md`: the OTA slot and rollback logic,
  `opadctl flash` and `opadctl recover`
  (`desktop/cli/src/main.rs`, `desktop/crates/opad-device/src/flash.rs`,
  `desktop/daemon/src/firmware_update.rs`,
  `desktop/crates/opad-update/src/`, the firmware's OTA and USB code). A
  user must never be able to follow these pages into a stranded pad, and
  nothing may mention pressing BOOT or RESET.
- `docs/explanation/app-and-pad-versions.md` and
  `docs/reference/serial-protocol.md`: framing negotiation and
  old-app/new-pad compatibility, against `protocol/opad.proto`,
  `desktop/crates/opad-protocol/` and `firmware/main/protocol/` and `usb/`.
- Touch (AGENTS.md §1.4) and Hall Effect calibration
  (`docs/specs/v2-hall-effect-decisions.md` D7, `firmware/main/input/`).
- `docs/explanation/latency.md`: no claim about latency that the code or a
  measurement in the repo does not support.
- The section numbers that code prints (`git grep -n "docs/recovery.md\|docs/troubleshooting.md"`):
  each cited § must still contain what the message sends the user there
  for.

Resolve every `VERIFY(L3)` marker: confirm, correct or delete. None may
remain at the end.

# 2. The whole set

1. **Structure.** Read `docs/index.md` and every page's frontmatter as the
   sidebar would show it. Check that: Diátaxis types are not mixed within a
   page; a user can go from `index.md` to a working pad without touching
   `develop/`; nothing important is missing (compare with the topics in
   `docs/README.md`, the old docs layer 1 deleted (`git log --diff-filter=D
   --name-only main..docs-wiki -- docs`) and FINDINGS); no two pages cover
   the same thing. Fix what you can: merge, split, move (updating links), or
   delete.
2. **Old content.** Every deleted old page: confirm that what was true in it
   now lives somewhere. `git show main:docs/<file>` lets you read it.
3. **Consistency.** One name per concept across pages, the same terminology
   as the GUI and `opadctl`, consistent `sidebar.order`, useful
   `description`s (they become search snippets).
4. **Spanish.** Read the Spanish versions of the critical pages fully and
   sample the rest: natural neutral Spanish, `tú`, glossary followed, the
   same meaning as the English page.
5. **`docs/README.md`.** A short contributor map: what lives where, the
   citation and fixed-path rules, how to add a language (a new `docs/<lang>/`
   mirror plus the site's locale config), and that `docs/history/` is the
   archive.
6. **Links from outside `docs/`.** `git grep -n "docs/" -- ':!docs'`: every
   path that `README.md`, `AGENTS.md`, `SECURITY.md`, the `Makefile`, CI
   and the code mention must still exist. If one moved, do not edit code.
   List it for the owner instead (see §3).

# 3. What you must not decide alone

Write these into the final report as questions with a recommendation. Do not
do them:

- changing code, including user-facing strings that print a docs path (for
  example, making error messages point to the website instead of
  `docs/recovery.md`);
- fixing bugs from FINDINGS;
- editing `README.md` or `AGENTS.md` (propose the exact diff instead);
- anything in the separate website repo.

# 4. Finish

1. `check_docs.py`: 0 errors, and every remaining warning explained.
2. `make check` still passes (docs should not affect it; run it anyway).
3. Delete `docs-work/` (`git rm -r docs-work`) **only after** copying
   `FINDINGS.md` and the open questions into the final report. The work
   files are not part of the docs.
4. Commit following AGENTS.md §3 (`docs(...)`, `AI: generated`, the owner's
   identity, no model co-author line).
5. Give the owner a final report in Spanish:
   - what is published and the page count per section and per language;
   - what you corrected yourself in the critical pages;
   - the FINDINGS (code bugs or doubts), numbered, each with the files involved;
   - the questions from §3, each with a recommendation;
   - the steps left for the owner: review `git diff main..docs-wiki --stat`,
     fast-forward merge, delete the branch and worktree, and the website-side
     work (Starlight content loader pointing at `docs/` excluding
     `history/**` and `README.md`, locales `root: en` and `es`, the remark
     link plugin, Mermaid plugin, a `repository_dispatch` rebuild on pushes
     to `main` that touch `docs/`).
