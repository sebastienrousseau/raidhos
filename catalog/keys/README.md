# Bundled distro signing keys

This directory holds ASCII-armoured GPG public keys used by the
catalog to verify `SHA256SUMS` signatures. Each file is named
`<fingerprint>.asc` (long form, no spaces) so the catalog manifest
can reference it directly.

## How to add a key

1. Locate the distro's published signing key (look in the distro's
   "verify your download" documentation).
2. Verify the fingerprint **out-of-band** against at least two
   independent sources (distro site, Wikipedia article, key servers
   like `keys.openpgp.org`, prior known-good keys in your keyring).
3. Save the ASCII-armoured key as `<fingerprint>.asc`.
4. Commit alongside the catalog entry that references it.

## How to update a key

Keys do get rotated. When a distro publishes a new signing key:

1. Verify the new key the same way as above.
2. Add the new `.asc` file.
3. Update the catalog entry's `gpg_fingerprint`.
4. Keep the old key file for one release cycle so users on the previous
   release can still verify.

## Why bundle rather than fetch?

A network-fetched keyring is a TOFU (trust on first use) operation;
the first download is unverifiable against anything more authoritative
than DNS+TLS. Bundling pins the trust to a specific git commit, which
is signed by the maintainer and reviewable by anyone.
