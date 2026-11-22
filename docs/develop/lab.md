---
title: Automated Testing Lab
description: Self-hosted multi-OS VirtualBox testing lab architecture, VM topology, install/upgrade/uninstall validation, and health checks.
sidebar:
  order: 8
---

OPad validates installers, upgrades, and clean uninstallation across Linux and Windows in a self-hosted VirtualBox lab ([/scripts/lab/lab-common.ps1](/scripts/lab/lab-common.ps1)).

## 1. Host Architecture & Security Model

The lab runs on a Windows 11 host with Oracle VirtualBox (`C:\opad-lab`), registered as a self-hosted GitHub Actions runner with labels `[self-hosted, opad-lab]`.

```mermaid
flowchart TD
    GH["GitHub Actions (lab-install.yml / lab-check.yml)"] -->|"runs-on: [self-hosted, opad-lab]"| Host["Windows 11 Host (VirtualBox)"]
    Host --> Snap["Restore Clean 'limpia' Snapshot"]
    Snap --> Boot["Boot Headless VMs"]
    Boot --> Linux["Ubuntu 22.04 / 24.04 / 26.04, Fedora 43, Arch"]
    Boot --> Win["Windows 10 / Windows 11"]
    Linux --> LTest["vm-install-linux.sh over SSH"]
    Win --> WTest["vm-install-windows.ps1 over SSH"]
```

> [!IMPORTANT]
> **No Pull Request Execution**: The self-hosted runner never executes untrusted pull requests or forks. It runs only on official release / `main` workflows ([/.github/workflows/lab-install.yml](/.github/workflows/lab-install.yml), [/.github/workflows/lab-check.yml](/.github/workflows/lab-check.yml)).

## 2. Target VM Topology

Defined in `$AllVms` ([/scripts/lab/lab-common.ps1](/scripts/lab/lab-common.ps1)):

| VM Name | Guest OS | Package Format | Forwarded SSH Port |
|---|---|---|---|
| `ubuntu-22.04` | Ubuntu 22.04 LTS | `.deb` | `2222` |
| `ubuntu-24.04` | Ubuntu 24.04 LTS | `.deb` | `2224` |
| `ubuntu-26.04` | Ubuntu 26.04 LTS | `.deb` | `2226` |
| `fedora-43` | Fedora 43 | `.rpm` | `2243` |
| `arch` | Arch Linux | `opad-bin` / `pacman` | `2250` |
| `windows-10` | Windows 10 | `opad-setup.exe` | `2310` |
| `windows-11` | Windows 11 | `opad-setup.exe` | `2311` |

Each VM auto-logs into an `opad` desktop session (`seat0` on Linux, active console on Windows) with passwordless `sudo` / Administrator elevation over loopback SSH (`127.0.0.1`).

## 3. Automated Install, Upgrade, and Uninstall Lifecycle

Driven by [/scripts/lab/lab-install.ps1](/scripts/lab/lab-install.ps1), [/scripts/lab/vm-install-linux.sh](/scripts/lab/vm-install-linux.sh) (normalized to LF line endings before upload), and [/scripts/lab/vm-install-windows.ps1](/scripts/lab/vm-install-windows.ps1):

1. **Clean Baseline**: Restores the `limpia` snapshot, boots headless until the desktop session is ready, installs the baseline release, and reboots to verify autostart.
2. **In-Place Upgrade (`-Feed` or direct)**:
   - When invoked with `feed: true` (from `release-app.yml`, unpacking the not-yet-deployed `github-pages` artifact) or `deployed_feed: true` (manual dispatch), `lab-install.ps1` copies `beta.json` and `beta.json.minisig` into the VM and runs the `update` phase: serves the feed on loopback (`127.0.0.1:8000`), restarts `opad-daemon` with `OPAD_MANIFEST_URL=http://127.0.0.1:8000/beta.json`, and asks it over IPC to switch to `beta`, check, and install. On Ubuntu/Fedora/Windows OPad's own updater downloads, verifies, and installs the candidate release (through `pkexec` or `/SILENT`); on Arch the updater must report `notify_only` and `makepkg` installs the package directly. If the updater fails, the run is marked failed and the package is installed directly so the remaining checks still execute.
   - Without `-Feed`, installs the candidate package directly. Reboots and verifies binary version (`FILES.sha256`), running `opad-daemon` and tray GUI, `opadctl status` (decoding missing-DLL NTSTATUS codes on Windows), `cap_sys_ptrace=eip` on `.deb`/`.rpm` `tosu`, and persistence of `opad.db` (`~/.local/share/opad/opad.db` on Linux, `%APPDATA%\opad\opad.db` on Windows).
3. **Clean Uninstallation**: Uninstalls the package/installer and verifies removal of binaries (`/usr/bin/` on Linux, `%LOCALAPPDATA%\Programs\opad` on Windows), autostart/systemd files, `udev` rules, background processes, and any `tosu` directories carrying OPad's `.created-by-opad` marker (unmarked directories are kept and noted with `info:`), then powers off and restores `limpia`.

## 4. On-Demand Health Checks

[/.github/workflows/lab-check.yml](/.github/workflows/lab-check.yml) boots VMs from `limpia`, verifies SSH/session readiness, captures desktop screenshots via `VBoxManage controlvm screenshotpng`, and publishes summaries with [/scripts/lab/lab-results.ps1](/scripts/lab/lab-results.ps1) (which also marks any prior orphaned `"running"` run as failed with an `Interrupted` group and records `info:` lines).
