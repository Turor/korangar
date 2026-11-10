# Signed client auto-update

The `korangar-update` crate checks a signed release manifest at startup. Nothing is downloaded unless `KORANGAR_UPDATE_MANIFEST_URL` is set. `KORANGAR_SKIP_UPDATE=1` disables the hook even then.

Allowed fetches: `file://` (tests), loopback `http://` (`127.0.0.1`, `localhost`, `::1`), and `https://ro.turoran.com/...` only. Any other host is rejected. This tree does not publish a manifest and does not change live hosting.

`EMBEDDED_UPDATE_PUBLIC_KEY` in `korangar-update/src/manifest.rs` is the Turoran release public key (`fb3dd3d6b9e1955ee03a4786596f7d5562b7d96d7b6c177e6d38cf7abca7de18`). An all-zero key still fails closed. The matching private seed stays on the signing controller. Do not commit a private seed.

## Canonical payload (what is signed)

Ed25519 signs the UTF-8 payload below, not the envelope. Files are sorted by path bytes. There is no insignificant whitespace.

```text
KRUP1
<version>
<file count>
for each file:
<relative path>
<sha256 lowercase hex>
<size decimal>
<url>
```

`version` matches `[0-9A-Za-z._+-]{1,64}`. Paths are relative, use `/`, and cannot contain `.`, `..`, or an `updates/` prefix. The on-disk envelope is:

```text
KRUP1SIG
<128 lowercase hex signature bytes>
<canonical payload>
```

Each downloaded file must match the declared size and SHA-256. Any failure deletes the staging directory and leaves the install unchanged.

## Apply

Files land in `<install>/updates/staging/<version>/`, then each existing target is renamed to `<name>.old` and the staged file takes its place. An existing `<name>.old` is renamed to `<name>.old.aside` before the live file moves. If that rename fails, or any later swap, self-check, or relaunch fails, files already replaced in this apply are restored and any aside backup is moved back to `<name>.old`. On Windows this is the supported way to replace a running `korangar.exe` (rename the running image, then move the new image onto the original name). The new executable is started with `--update-self-check` (and `KORANGAR_SKIP_UPDATE=1`). After success the client relaunches, the parent exits, and leftover `.old.aside` files are removed. `updates/applied.version` records the manifest version so a relaunch does not loop when the binary's package version was not bumped.

## Keygen and signing

Run these outside the repository. The CLI refuses to write a seed under a directory that contains `.git`.

```bash
korangar-sign-release keygen --out /secure/korangar-update.seed
korangar-sign-release public-key --private-key /secure/korangar-update.seed
```

The client already embeds the controller's public key. Re-print it with `korangar-sign-release public-key` to confirm a seed matches. Do not replace the const with a different key unless the controller key is rotated on purpose.

`KORANGAR_UPDATE_SIGNING_KEY` may be a 64-character hex seed or a path to a 32-byte seed file. Prefer the path. The helper below does not print the seed.

```bash
scripts/sign-korangar-release.sh \
  --private-key /secure/korangar-update.seed \
  --version 0.1.2 \
  --out /secure/manifest.krup \
  --file korangar.exe=/path/to/korangar.exe \
  --file runtime/note.txt=/path/to/note.txt --url runtime/note.txt
```

`--private-key` may be omitted when `KORANGAR_UPDATE_SIGNING_KEY` is set. The script runs `korangar-sign-release` from `KORANGAR_SIGN_BIN` or `PATH`, otherwise `cargo run -p korangar-update --bin korangar-sign-release`. On the shared dev host, build through `/root/Work/korangar-parity-swarm-20261109/cargo-run.sh` and point `KORANGAR_SIGN_BIN` at that binary.

The same arguments work if you call `korangar-sign-release sign` directly.

The default artifact URL is the relative path, resolved against the manifest URL's directory.

## Hosting proposal (not applied)

Do not change HAProxy, remoteclient2, or DNS from this change. Do not publish a manifest.

Proposed public base, once an operator chooses to serve it:

`https://ro.turoran.com/client/releases/`

That host already terminates HTTPS for the game client assets. A later change can place release files on the existing remoteclient2 document root (for example `client/releases/manifest.krup` and the named artifacts) or add a path on the existing HAProxy HTTPS frontend for `ro.turoran.com`. No new public hostname.

Clients would then set:

`KORANGAR_UPDATE_MANIFEST_URL=https://ro.turoran.com/client/releases/manifest.krup`

Relative file URLs in the manifest stay under that directory. The updater will fetch them only when the host is exactly `ro.turoran.com`. Tests keep using `file://` and loopback `http://`. Until a manifest is published, leave `KORANGAR_UPDATE_MANIFEST_URL` unset so startup does not contact the site.
