# Signed client auto-update

The `korangar-update` crate checks a signed release manifest at startup. Nothing is downloaded unless `KORANGAR_UPDATE_MANIFEST_URL` is set. `KORANGAR_SKIP_UPDATE=1` disables the hook even then. This tree does not point at a CDN, and non-loopback `http` hosts plus every `https` URL are rejected.

`EMBEDDED_UPDATE_PUBLIC_KEY` in `korangar-update/src/manifest.rs` is 32 zero bytes. Signature checks fail closed until a release build replaces that const. Do not commit a private seed.

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

Paste the printed `EMBEDDED_UPDATE_PUBLIC_KEY` into `korangar-update/src/manifest.rs` for that release build only.

`KORANGAR_UPDATE_SIGNING_KEY` may be a 64-character hex seed or a path to a 32-byte seed file. Prefer the path.

```bash
korangar-sign-release sign \
  --private-key /secure/korangar-update.seed \
  --version 0.1.2 \
  --out /secure/manifest.krup \
  --file korangar.exe=/path/to/korangar.exe \
  --file runtime/note.txt=/path/to/note.txt --url runtime/note.txt
```

The default artifact URL is the relative path, resolved against the manifest URL's directory (`http://127.0.0.1` or `file://` only in this build).
