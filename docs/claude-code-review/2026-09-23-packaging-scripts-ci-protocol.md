# Code review: `packaging`, `scripts`, `.github`, `protocol`

- Date: 2026-09-23
- Command: `/code-review medium packaging scripts .github protocol`
- Base: `main` @ `b0e0a37`
- Scope: no diff touched these paths, so the review covers their current contents. `protocol/` (`osupad.proto`, `.options`) came out clean — every string/repeated field has a nanopb bound.
- Verification: findings were not re-verified after the review; treat each as *plausible* until checked.
- Status legend: `open` / `fixed` / `wontfix` / `invalid`

---

## 1. `build_release.sh` signs a manifest over a locally rebuilt `dist/` — `open`

**File:** `scripts/release/build_release.sh:128`
**Category:** correctness

Signs the release manifest over a locally rebuilt `dist/` (after `rm -rf dist` at line 17), so the hashes can't match the zigbuild artifacts `release.yml` publishes; run after `build_packages.sh` it also deletes the .deb/.rpm/AppImage before signing. Updater verification against the published files would fail.

## 2. `AppRun` leaves a trailing empty `LD_LIBRARY_PATH` entry — `fixed`

**File:** `packaging/linux/appimage/AppRun:6`
**Category:** security

`LD_LIBRARY_PATH=...:${LD_LIBRARY_PATH}` leaves a trailing empty entry when the variable is unset, which the loader treats as the current directory; a planted `.so` in the launch directory gets loaded.

**Suggested fix:** Use `${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}`.

## 3. `test_packages.sh` pre-installs `libcap2-bin` the .deb does not depend on — `open`

**File:** `scripts/release/test_packages.sh:26`
**Category:** correctness

Installs `libcap2-bin`/`procps` before the package, while the .deb has no `libcap2-bin` Depends and postinst does `setcap ... || true`; the cap_sys_ptrace check passes in the test but a host without `setcap` silently gets a tosu that can't read osu! memory.

## 4. Arch `PKGBUILD` never builds or depends on espflash — `open`

**File:** `packaging/linux/arch/PKGBUILD:41`
**Category:** correctness

Never runs `make espflash` and has no espflash dependency, so the Arch install has no `/usr/lib/opad/bin/espflash`; `flash.rs` falls back to a bare `espflash` on PATH, which isn't there, so firmware flashing fails and the version pin is lost.

## 5. Release glibc assertion skips the tosu binary — `open`

**File:** `.github/workflows/release.yml:154`
**Category:** correctness

The in-package glibc assertion skips `usr/lib/opad/tosu/tosu`, the one binary rebuilt for portability, and the distro matrix bottoms out at glibc 2.34/2.35; a tosu regressing to need glibc 2.32-2.35 passes both gates despite the 2.28 target stated on line 102.
