# Security policy

## Reporting a vulnerability

Report it privately through GitHub: **Security → Report a vulnerability** on
<https://github.com/OPad-dev/OPad/security/advisories/new>. Please do not open
a public issue, post it on Discord, or share an exploit before a fix is out.

You get an answer within 7 days. A fix for a confirmed issue ships in the next
release of the part it affects (app, firmware or bundled tosu), and you are
credited in the advisory unless you ask not to be.

## What is in scope

- The update path: the signed feed, its signature check, downloads, and how
  the app and the firmware are installed (`desktop/crates/opad-update`).
- The desktop daemon's local IPC (who may talk to it) and the serial
  protocol between the app and the pad.
- The firmware: anything that makes a pad send a key nobody pressed, stop
  working as a keyboard, or fail to recover over USB.
- The release workflows in `.github/workflows/`.

The committed test key in `desktop/crates/opad-update/test-key/` is public on
purpose and protects nothing; it is replaced by the release key before the
first stable release (`docs/specs/release-and-updates.md` §5.4).

## Supported versions

Only the latest app release and the latest firmware release get fixes.
