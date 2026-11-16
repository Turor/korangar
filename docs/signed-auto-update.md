# Signed client auto-update

The `korangar-update` crate checks a signed release manifest at startup. The client checks the main-channel manifest at startup. The default URL is `https://ro.turoran.com/client/korangar/main/latest`. Override it with `KORANGAR_UPDATE_MANIFEST_URL`. `KORANGAR_SKIP_UPDATE=1` disables the hook. An unreachable channel is skipped so the current binary still starts. A bad signature, a failed file hash, or a manifest whose build is not newer than the installed one is fatal.

Allowed fetches: `file://` (tests), loopback `http://` (`127.0.0.1`, `localhost`, `::1`), `https://ro.turoran.com/client/korangar/main/latest`, and presigned `https://garage.turoran.com/korangar-releases/main/<allowlisted-name>` URLs (SigV4 query). Any other host or path is rejected. The signing seed is never in this tree.

`EMBEDDED_UPDATE_PUBLIC_KEY` in `korangar-update/src/manifest.rs` is the Turoran release public key (`fb3dd3d6b9e1955ee03a4786596f7d5562b7d96d7b6c177e6d38cf7abca7de18`). An all-zero key still fails closed. The matching private seed stays on the signing controller. Do not commit a private seed.

## Canonical payload (what is signed)

Ed25519 signs the UTF-8 payload below, not the envelope. Files are sorted by path bytes. There is no insignificant whitespace.

```text
KRUP1
<version>
<build>
<file count>
for each file:
<relative path>
<sha256 lowercase hex>
<size decimal>
<url>
```

`version` matches `[0-9A-Za-z._+-]{1,64}`. `build` is a positive decimal integer (monotonic counter or unix timestamp). The updater rejects a signed manifest whose build is not strictly newer than `updates/applied.build` (missing file counts as 0). Paths are relative, use `/`, and cannot contain `.`, `..`, or an `updates/` prefix. The on-disk envelope is:

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
  --build "$(date +%s)" \
  --sha "$(git rev-parse HEAD)" \
  --out /secure/manifest.krup \
  --linux /path/to/korangar \
  --windows /path/to/korangar.exe
```

`--linux` and `--windows` are each optional. At least one is required (a `--file` whose name is `korangar-linux-*` or `korangar-windows-*.exe` counts). Omitting both refuses an empty manifest. `--dry-run` prints the file list and the key path and does not read the seed or write a signature:

```bash
scripts/sign-korangar-release.sh --dry-run --sha "$(git rev-parse HEAD)" --linux /path/to/korangar
```

`--private-key` may be omitted when `KORANGAR_UPDATE_SIGNING_KEY` is set. The script runs `korangar-sign-release` from `KORANGAR_SIGN_BIN` or `PATH`, otherwise `cargo run -p korangar-update --bin korangar-sign-release`. On the shared dev host, build through the gap-swarm `cargo-run.sh` and point `KORANGAR_SIGN_BIN` at that binary. Never print the seed; dry-run prints only the key path (a raw 64-hex env value is reported as not printed).

The same arguments work if you call `korangar-sign-release sign` directly.

The default artifact URL is the relative path, resolved against the manifest URL's directory.

## Hosting (Garage)

Releases live in the dedicated Garage bucket `korangar-releases`, prefix `main/`. The bucket is **private** (no anonymous or website access). This is separate from the Classless WoW `wow` bucket.

The client does not fetch the bucket directly. It calls:

`https://ro.turoran.com/client/korangar/main/latest`

remoteclient2 returns short-lived (~10 minute) presigned GET URLs at `https://garage.turoran.com` (the existing public S3 API) for `main/manifest.jsonup` and the allowlisted binaries named in that manifest. The client then verifies the signed KRUP1 envelope, file SHA-256 hashes, and rollback protection (`incoming build > installed build`) before swapping any files.

Object layout:

```
main/manifest.jsonup
main/korangar-linux-<gitsha>
main/korangar-windows-<gitsha>.exe
```

Release binaries keep their bucket object names in the manifest (`korangar-linux-<gitsha>`, `korangar-windows-<gitsha>.exe`) so remoteclient2 can allowlist and presign them. On apply, the client installs the binary for its own platform over the running executable (`bin/korangar`, `korangar.exe`, wherever it was started from) and skips the other platform's file without downloading it.

Linux and Windows share one main channel, and either platform may ship alone. A manifest with no file for the running platform is skipped: `updates/applied.build` stays unchanged, the other platform's file is not downloaded, and the next startup of that same manifest skips again (it does not reinstall). The client logs `release <version> has no <platform> build; staying on <current>`. A later release that includes this platform applies once, then reports up to date. Rollback protection is unchanged: a signed manifest whose build is not strictly newer than `applied.build` is still rejected. An empty manifest (no file entries) is refused by the signer and by publish.

The last three published SHAs are kept; older binaries are deleted after a successful publish.

The read-only Garage key used to mint those URLs lives on rodata-dev at `/etc/remoteclient2/korangar-releases-reader.env` (mode 600), loaded by systemd, not in the git tree and not in the web-served working folder. The controller holds the write key at `~/Work/korangar-signing/garage-writer.env`.

### Release pipeline (every merge to `main`)

```
merge → build Linux (gha-elastic, unsigned) → controller poller
                                              → Windows exe on win11-build VM 118
                                              → sign with host-local seed
                                              → publish to Garage korangar-releases/main/
```

1. **Build (CI, unsigned)**  
   Workflow `.github/workflows/main-release.yml` on `push` to `main`, `runs-on: [self-hosted, elastic]`. Builds the Linux x86_64 client and uploads an unsigned artifact. No signing key in the workflow, repo secrets, or ephemeral guests (VMID 900–999).

2. **Windows (win11-build VM 118)**  
   The controller poller (not the ephemeral guest) SSHs to `win11-build` (`192.168.11.62`, user `builder`) and builds `korangar.exe` with `nightly-2026-02-01` + `windows-test-deployment`. The seed never lands on 118.

3. **Sign (controller, host-local)**  
   systemd timer `korangar-release-publish.timer` on the controller polls the latest successful `main-release` run, fetches the Linux artifact, builds Windows if needed, then runs `scripts/sign-korangar-release.sh` with `--private-key /home/controller/Work/korangar-signing/update.seed` and a monotonic `--build`. The seed is never printed and never an Actions secret.

4. **Publish**  
   Upload via Garage S3 on garage-1 (`http://127.0.0.1:3900`, jump `root@192.168.11.3` → `root@192.168.11.20`) using a **write-only** (read+write, not owner) key limited to `korangar-releases`. Manifest is written to a temp key and renamed/copied to `main/manifest.jsonup` last. Keep the last 3 SHAs.

### Failures and manual publishes

- Either platform can ship alone. A failed Windows build does not hold back Linux, and a failed Linux run does not hold back Windows when that Windows build succeeded. `scripts/publish-korangar-main-channel.sh` takes `--linux` and `--windows` as optional flags and requires at least one. It refuses an empty manifest and a manifest that names a platform binary that is not being uploaded.
- `--dry-run` prints the manifest file list and the object keys (`main/korangar-linux-<sha>`, `main/korangar-windows-<sha>.exe`, `main/manifest.jsonup`) and does not contact Garage.
- Clients of the missing platform skip that release, log `release <version> has no <platform> build; staying on <current>`, and keep their current `applied.build`. They do not download the other platform's file.
- Each sha gets 3 publish attempts; the poller then ignores it until `~/Work/korangar-release-publisher/failures/<sha>` is deleted.
- Manual Linux-only publish: `SKIP_WINDOWS=1 ~/Work/korangar-release-publisher/poll-and-publish.sh`. Manual Windows-only publish (Linux run failed, Windows build succeeded): `SKIP_LINUX=1 ~/Work/korangar-release-publisher/poll-and-publish.sh`. The poller logs which platform is behind.
- `main-release` stays Linux-only on GitHub. It does not sign or upload. Windows is built by the controller poller on win11-build. A cancelled or failed Linux job is not a reason to add signing to Actions.

### Platform behind

When one platform's clients do not move and the other's do:

1. Read the client log for `release <version> has no linux build; staying on <current>` or the same line with `windows`. That client is healthy and still on its previous build.
2. Confirm the channel manifest lists only the other platform (`korangar-linux-<sha>` or `korangar-windows-<sha>.exe`). Dry-run the publish script against the signed manifest to print that list without uploading.
3. Do not republish the same build number with the missing binary added. Rollback protection treats an equal build as already current on clients that did apply it, and clients that skipped did not record the build. Ship a new monotonic `--build` that contains the missing platform (alone, or with the other side).
4. The skipped client applies that newer build once. A second startup of the same manifest is up to date and does not swap again.
5. On the controller, a Windows-only publish is the case where the GitHub Linux run failed and the win11-build binary succeeded. The poller should log `platform behind: linux` and upload only the Windows object. The reverse log line is `platform behind: windows`.

### Follow-ups (operator tasks, not code)

- Move the signing seed from `~/Work/korangar-signing/update.seed` on the controller into Bitwarden. Scripts and the embedded public key stay as they are.
- Enable auto-update on omarchy installs after a signed build is installed there (clear `KORANGAR_SKIP_UPDATE`). Not part of this change.

### What stays offline

- Ed25519 **private seed** and passphrase: controller `~/Work/korangar-signing/` only.
- Garage writer secret: controller `~/Work/korangar-signing/garage-writer.env` (mode 600). Never committed.
- Public key only is embedded in the client.

### Operator cheat-sheet

```bash
# on the controller, after artifacts are in /tmp/korangar-release-work
# either --linux or --windows may be omitted; at least one is required
./scripts/sign-korangar-release.sh --dry-run --sha "<sha>" --linux <linux-bin> --windows <windows-exe>
./scripts/sign-korangar-release.sh \
  --private-key "$HOME/Work/korangar-signing/update.seed" \
  --version "main-$(git rev-parse --short=12 HEAD)" \
  --build "$(date +%s)" \
  --sha "<sha>" \
  --out /tmp/manifest.jsonup \
  --linux <linux-bin> \
  --windows <windows-exe>
./scripts/publish-korangar-main-channel.sh --dry-run --sha "<sha>" --manifest /tmp/manifest.jsonup --linux <linux-bin> --windows <windows-exe>
# then the same command without --dry-run copies objects to Garage
```
