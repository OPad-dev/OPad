---
title: Automated Testing Lab
description: Overview of the self-hosted multi-OS virtual machine lab, runner isolation, automated installation testing, upgrade validation, and cleanup verification.
---

# Automated Testing Lab

To guarantee that installers, upgrade paths, and desktop integrations function flawlessly across diverse operating systems, OPad operates a dedicated, self-hosted physical testing laboratory.

This document describes the lab architecture, security constraints, virtual machine topology, and automated testing procedures.

<!-- src: /scripts/lab/lab-common.ps1#AllVms -->
<!-- src: /.github/workflows/lab-check.yml#opad-lab -->
<!-- src: /.github/workflows/lab-install.yml#concurrency -->
<!-- src: /scripts/lab/vm-install-linux.sh#installed_version -->

---

## 1. Hardware Architecture and Security Model

The lab runs on a dedicated Windows 11 mini PC registered as a self-hosted GitHub Actions runner with labels `[self-hosted, opad-lab]`.

```mermaid
flowchart TD
    GH["GitHub Actions Orchestrator\n(release-app.yml or manual dispatch)"] -->|Runs on [self-hosted, opad-lab]| MiniPC["Windows 11 Mini PC Host\n(Oracle VirtualBox 7.x)"]
    MiniPC --> Snap["Revert VM to Clean 'limpia' Snapshot"]
    Snap --> Boot["Boot Virtual Machines in Headless Mode"]
    Boot --> VM1["Ubuntu 22.04 / 24.04 / 26.04 (APT)"]
    Boot --> VM2["Fedora 43 (DNF)"]
    Boot --> VM3["Arch Linux (AUR opad-bin)"]
    Boot --> VM4["Windows 10 / Windows 11 (Inno Setup)"]
    VM1 --> Test["Run vm-install-linux.sh via SSH"]
    VM2 --> Test
    VM3 --> Test
    VM4 --> WinTest["Run vm-install-windows.ps1 via SSH"]
```

### Security Isolation Invariant

> [!IMPORTANT]
> **No Pull Request Execution**: The self-hosted runner **never** executes code from pull requests or forks. It only triggers on official release workflows ([.github/workflows/lab-install.yml](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/.github/workflows/lab-install.yml)) or manually dispatched workflows ([.github/workflows/lab-check.yml](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/.github/workflows/lab-check.yml)) on the `main` branch.

---

## 2. Target VM Topology

The lab maintains seven virtual machines managed via VirtualBox ([scripts/lab/lab-common.ps1](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/scripts/lab/lab-common.ps1#L11-L19)):

| VM Identifier | Guest OS | Package Format | Host SSH Port | Package Manager |
|---|---|---|---|---|
| `ubuntu-22.04` | Ubuntu 22.04 LTS | `.deb` | 2222 | `apt-get` |
| `ubuntu-24.04` | Ubuntu 24.04 LTS | `.deb` | 2224 | `apt-get` |
| `ubuntu-26.04` | Ubuntu 26.04 LTS | `.deb` | 2226 | `apt-get` |
| `fedora-43` | Fedora 43 | `.rpm` | 2243 | `dnf` |
| `arch` | Arch Linux | AUR `opad-bin` | 2250 | `pacman` / `makepkg` |
| `windows-10` | Windows 10 Pro | `.exe` | 2310 | Inno Setup |
| `windows-11` | Windows 11 Pro | `.exe` | 2311 | Inno Setup |

Each VM boots with auto-login into a non-privileged `opad` desktop session and provides passwordless `sudo` (Linux) or Administrator privileges (Windows) over forwarded loopback SSH ports.

---

## 3. Automated Installation and Upgrade Lifecycle

The primary test harness is driven by [scripts/lab/lab-install.ps1](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/scripts/lab/lab-install.ps1), which executes a three-phase test on each VM:

### Phase 1: Clean Baseline Installation
1. The host reverts the VM to its clean `limpia` snapshot using `VBoxManage snapshot restore`.
2. The VM boots until its graphical desktop environment is active (Linux: session on `seat0`; Windows: active console session).
3. The previous stable release package is transferred via SCP and installed.
4. The VM reboots to verify that autostart services launch properly.

### Phase 2: In-Place Upgrade Verification
1. The new release candidate package is transferred and installed.
2. The VM reboots into the desktop session.
3. Verification script ([scripts/lab/vm-install-linux.sh](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/scripts/lab/vm-install-linux.sh)):
   - Verifies the installed binary version matches the release tag.
   - Asserts that all installed files match the release checksums.
   - Validates that `opad-daemon` and the system tray GUI are running.
   - Tests daemon communication via `opadctl status`.
   - Confirms `tosu` has retained its `cap_sys_ptrace=eip` security capability.
   - Confirms user settings and database history (`~/.local/share/opad/opad.db` or `%LOCALAPPDATA%\opad\opad.db`) survived the upgrade intact.

### Phase 3: Clean Uninstallation
1. The package is uninstalled using the distribution package manager or Windows uninstaller.
2. The test verifies complete removal:
   - All installed binaries in `/usr/bin/` or `Program Files` are removed.
   - Autostart desktop files and systemd service units are deleted.
   - Udev rules are removed and reloaded.
   - No orphaned background processes remain running.

---

## 4. Health Checks and Diagnostics

The lab health can be checked on demand via [.github/workflows/lab-check.yml](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/.github/workflows/lab-check.yml):
- Verifies that the host Windows 11 session is running.
- Boots all configured VMs from snapshots in parallel or individually.
- Captures automated screenshots of each guest's desktop to detect UI regressions, storing them as workflow artifacts for 14 days.
- Publishes execution summaries using `scripts/lab/lab-results.ps1`.
