# Test key

A throwaway minisign key pair for the update feed, **public on purpose**: the
secret half (`opad-test.key`) has no passphrase and is committed, so anyone can
sign a feed with it. It protects nothing.

It exists so the whole update path, signature check included, works before
releases are signed for real (docs/specs/release-and-updates.md §5.4). Builds
trust it through `TEST_KEY` in `src/verify.rs`; `update-feed.yml` and
`make feed-local` sign with it until the real feed key is set up (§19), after
which it is dropped.
