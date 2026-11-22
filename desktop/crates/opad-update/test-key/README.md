# Test key

A throwaway minisign key pair for the update feed, **public on purpose**: the
secret half (`opad-test.key`) has no passphrase and is committed, so anyone can
sign a feed with it. It protects nothing.

It exists so the whole update path, signature check included, can be tested
without the real feed key (docs/specs/release-and-updates.md §5.4). Builds
from 1.0.0-rc.6 trust it only where asked to (`test_key_allowed` in
`src/verify.rs`): `OPAD_TRUST_TEST_KEY=1` for the daemon, GUI and `opadctl`,
and for the root update helper only the root-owned file
`/etc/opad/trust-test-key`, which the lab's VMs create. Trusting it anywhere
else would let any program become root through the helper.

`update-feed.yml` still signs `<channel>.json.minisig` with it, for builds up
to 1.0.0-rc.5, which trust nothing else; `make feed-local` signs both
signature files with it.
