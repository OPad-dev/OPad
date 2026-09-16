# osu!pad on Windows — as built

**Status:** the code is written and compiles; **none of it has been run on
Windows hardware.** `docs/testing-checklist.md` §5 is the list of things that
have to happen before any of this is a claim rather than a design. Read this
document as "what the code does", not "what has been verified".

This replaces the earlier aspirational version of this file, which described a
port that had not been written yet. Where the two disagree, this one is right;
where this one disagrees with the code, the code is right.

The invariant first, because everything else is subordinate to it: **the pad is
a 1000 Hz HID keyboard on a Windows machine that has never seen the installer.**
Nothing here gates, delays or adds work to that path. Every mechanism below
lives on the CDC side.

---

## 1. Drivers: there are none to install

| Interface | Windows driver | Notes |
|---|---|---|
| HID keyboard | `hidclass.sys` / `kbdhid.sys`, inbox | `bInterval = 1`, so 1000 Hz is honoured natively |
| CDC-ACM | `usbser.sys`, inbox on Win10 1709+ | Appears as `COMn` |

No `.inf`, no WinUSB, no Zadig, and **zero firmware changes** — the transport
decision in §0 of the packaging plan was to keep CDC-ACM precisely so this
stayed true. It is also latency-irrelevant: CDC is a separate USB endpoint and
protocol traffic never touches the keystroke path.

---

## 2. Serial port discovery — shared, not ported

`desktop/crates/osupad-device/src/lib.rs` filters `serialport::available_ports()`
by VID/PID. The same code path returns `/dev/ttyACM0` on Linux and `COM3` on
Windows, and `find_target_port` (the app, `303a:4001`) and
`find_bootloader_port` (the ROM bootloader, `303a:1001`) are both unchanged.

Two Windows properties of that crate matter downstream:

- It opens with `share_mode = 0`, so **a COM handle is exclusive**. A process
  still holding the port does not merely slow a flash down, it fails it. This is
  why `PrepareFlash` exists and why the daemon drops its handle *before* it
  clears `is_port_open`.
- `COMPort::open` prefixes `\\.\` itself, so **COM10 and above work** with no
  special handling in our code.

---

## 3. IPC: Unix socket and named pipe behind one type

The framing (`[len: u32 LE][json]`) and the `IpcRequest` / `IpcResponse` enums
are transport-agnostic and identical on both platforms. What differs is the
concrete type, and `osupad-ipc` exposes it as three `#[cfg]`-selected aliases:

| Alias | Unix | Windows |
|---|---|---|
| `IpcStream` | `tokio::net::UnixStream` | `NamedPipeClient` |
| `IpcServerStream` | `UnixStream` | `NamedPipeServer` |
| `IpcListener` | wraps `UnixListener` | creates one pipe instance per accept |

Addresses:

- Unix: `$XDG_RUNTIME_DIR/osupad/daemon.sock`, or `/tmp/osupad-<uid>/daemon.sock`.
- Windows: `\\.\pipe\osupad-ipc-<user SID>`. The SID is in the name so two users
  on one machine get separate pipes, mirroring the per-uid socket path.

The accept loops are written separately on purpose. A `NamedPipeServer` is
consumed when a client connects and a fresh instance must be created for the
next one, which does not fit the Unix accept-loop shape and was not forced into
it.

### Security (§W0-2)

P2-6 hardened the Unix socket with `0700` and a peer check. A named pipe created
with default security is reachable by any process on the machine, including
across sessions, so the Windows path must not silently be weaker:

- Every pipe instance is created with an explicit DACL, built fresh per
  instance: `D:P(A;;GA;;;SY)(A;;GA;;;<user SID>)` — SYSTEM and the calling user,
  nobody else. `P` makes it protected, so it does not inherit anything.
- If the SID cannot be determined, `create_listener` **fails** rather than
  creating an unrestricted pipe.
- The first server instance uses `first_pipe_instance(true)`, so a second daemon
  fails loudly instead of squatting the pipe. That is the Windows half of the
  single-daemon guarantee; `ERROR_PIPE_BUSY` and `ERROR_ACCESS_DENIED` map to
  the same "another daemon is running" error the Unix path returns.

The two-account check ("a process running as a different user cannot open the
pipe") cannot be automated in CI and is WIN-03 in the checklist.

---

## 4. Paths

Everything resolves through `osupad_model::paths`, which uses `dirs` and
**fails loudly rather than falling back to a relative path**:

| What | Linux | Windows |
|---|---|---|
| `osupad.db` | `~/.local/share/osupad/` | `%APPDATA%\osupad\` |
| Logs (`daemon.log`, `tosu.log`) | `~/.local/state/osupad/` | `%APPDATA%\osupad\` |
| Install prefix | `<prefix>/lib/osupad` | the install directory itself |
| Bundled tosu | `<prefix>/lib/osupad/tosu/` | `<install dir>\tosu\` |
| `install-origin` marker | `<prefix>/lib/osupad/` | `<install dir>\` |

Windows and macOS have no XDG-style state directory, so logs live with the data
rather than in a second place the uninstaller would have to know about.

---

## 5. Startup at login

`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, written from
`gui/src/platform_windows.rs` via `winreg`. Task Scheduler was considered and
rejected: it needs elevation to create the task, and nothing here needs
privileges.

The installer must write these two values **byte for byte**, because the GUI
compares against them to decide whether autostart is on:

```
osupad-daemon = "<dir>\osupad-daemon.exe"
osupad-gui    = "<dir>\osupad-gui.exe" --tray
```

---

## 6. Hotplug

A reconnect poll, not `WM_DEVICECHANGE`: a 400 ms port scan with a 300 ms
settle, which keeps the 2 s acceptance budget with margin. §W1-2 allows this,
and it matters that a headless daemon then needs no window and no message pump.

---

## 7. GUI and tray

`iced` renders on Windows through wgpu (DX12 / Vulkan). The tray is
`cfg`-split — `ksni` (D-Bus StatusNotifierItem) on Linux, `tray-icon`
(`Shell_NotifyIcon`) on Windows — which is W0-5, on the `v1.0-agy` branch.

---

## 8. Flashing (§W1-3)

`osupadctl flash` drives `espflash`; there is no shell script on either
platform any more. Three things are Windows-specific:

1. **Line state is set explicitly.** Linux asserts DTR when a tty is opened;
   on Windows `serialport`'s DCB sets `fDtrControl = Disable` and leaves DTR
   low. The old code inherited whichever the platform did. Bootloader entry is
   now an explicit ladder of the three triggers the firmware accepts — the
   plain-text `BOOTLOADER` command, the 1200-baud touch, and the esptool
   DTR/RTS pattern — each setting DTR and RTS by hand.
2. **The daemon releases the port first**, over IPC (`PrepareFlash`), rather
   than by stopping a systemd unit. Exclusive handles make this mandatory.
3. **`reset_to_app` retries its open** for up to 3 s, because Windows can still
   be releasing espflash's handle when we go to reboot the pad.

`docs/recovery.md` §7 covers the manual path, including BOOT + RESET.

---

## 9. What is not done

- Nothing in this document has run on Windows. `docs/testing-checklist.md` §5.
- Code signing: the installer ships unsigned, so SmartScreen will warn. §W2-2
  is a SignPath Foundation application and needs the repository published
  first.
- `scripts/bench_latency.py` is evdev and has no Windows equivalent; see
  `docs/latency-testing.md` §4b.
