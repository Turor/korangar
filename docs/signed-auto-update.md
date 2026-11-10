# Signed client auto-update

The `korangar-update` crate checks a signed release manifest at startup. The client checks the main-channel manifest at startup. The default URL is `https://korangar-releases.turoran.com/main/manifest.jsonup`. Override it with `KORANGAR_UPDATE_MANIFEST_URL`. `KORANGAR_SKIP_UPDATE=1` disables the hook. An unreachable channel is skipped so the current binary still starts. A bad signature, a failed file hash, or a manifest whose build is not newer than the installed one is fatal.

Allowed fetches: `file://` (tests), loopback `http://` (`127.0.0.1`, `localhost`, `::1`), `https://ro.turoran.com/...`, and `https://korangar-releases.turoran.com/...`. Any other host is rejected. The signing seed is never in this tree.

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
  --out /secure/manifest.krup \
  --file korangar.exe=/path/to/korangar.exe \
  --file runtime/note.txt=/path/to/note.txt --url runtime/note.txt
```

`--private-key` may be omitted when `KORANGAR_UPDATE_SIGNING_KEY` is set. The script runs `korangar-sign-release` from `KORANGAR_SIGN_BIN` or `PATH`, otherwise `cargo run -p korangar-update --bin korangar-sign-release`. On the shared dev host, build through `/root/Work/korangar-parity-swarm-20261109/cargo-run.sh` and point `KORANGAR_SIGN_BIN` at that binary.

The same arguments work if you call `korangar-sign-release sign` directly.

The default artifact URL is the relative path, resolved against the manifest URL's directory.

## Hosting (Garage)

Releases live in the dedicated Garage bucket `korangar-releases`, prefix `main/`. This is separate from the Classless WoW `wow` bucket and from the remoteclient2 `/client/` tree.

Public URL (intended, HTTPS):

`https://korangar-releases.turoran.com/main/manifest.jsonup`

Object layout:

```
main/manifest.jsonup
main/korangar-linux-<gitsha>
main/korangar-windows-<gitsha>.exe
```

Relative artifact URLs resolve under that directory. The last three published SHAs are kept; older binaries are deleted after a successful publish.

### Public HTTPS (required, not yet live)

Garage already has a TLS S3 API vhost at `https://garage.turoran.com` (port 3900). Anonymous S3 access is denied (`Forbidden: Garage does not support anonymous access yet`). Public reads use the **website** endpoint (port **3902**) after `garage bucket website --allow`, as `korangar-releases.web.garage-1.turoran.com`. That hostname is not on public DNS, and nothing currently fronts port 3902 on HTTPS.

Needed before players can fetch the default URL (do not change HAProxy or DNS without a further explicit approval):

1. DNS: `korangar-releases.turoran.com` → the existing public VIP (today `garage.turoran.com` / `ro.turoran.com` resolve to 192.168.11.7).
2. HAProxy TLS vhost for that name, backend `192.168.11.20:3902` (and garage-2/3 if desired), `Host: korangar-releases.web.garage-1.turoran.com`.
3. Do **not** point this hostname at port 3900.

Until that vhost exists, the signer still publishes to the bucket. The client default URL is set; an unreachable channel is skipped.

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

### What stays offline

- Ed25519 **private seed** and passphrase: controller `~/Work/korangar-signing/` only.
- Garage writer secret: controller `~/Work/korangar-signing/garage-writer.env` (mode 600). Never committed.
- Public key only is embedded in the client.

### Operator cheat-sheet

```bash
# on the controller, after artifacts are in /tmp/korangar-release-work
./scripts/sign-korangar-release.sh   --private-key "$HOME/Work/korangar-signing/update.seed"   --version "main-$(git rev-parse --short=12 HEAD)"   --build "$(date +%s)"   --out /tmp/manifest.jsonup   --file korangar-linux-<sha>=<linux-bin>   --file korangar-windows-<sha>.exe=<windows-exe>
# then scripts/publish-korangar-main-channel.sh copies objects to Garage
```
