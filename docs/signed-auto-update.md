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

## Hosting proposal (draft — not live)

**Status:** design only. Do not publish manifests, change remoteclient2/HAProxy, or enable CI publish until Matthew approves.

### Goal

Korangar tracks `main` as a cutting-edge client. Every merge to `main` produces signed Linux + Windows builds on a **`main` channel**. On launch, the auto-updater pulls the newest `main` build (verify Ed25519 + SHA256, atomic swap, relaunch).

### Public layout (proposed)

Base (existing HTTPS vhost):

`https://ro.turoran.com/client/releases/`

Channel layout:

```
/client/releases/main/manifest.jsonup
/client/releases/main/korangar-linux-<gitsha>
/client/releases/main/korangar-windows-<gitsha>.exe
```

Optional later: `stable/`, `nightly/` with the same shape. Default client env / setting:

`KORANGAR_UPDATE_MANIFEST_URL=https://ro.turoran.com/client/releases/main/manifest.jsonup`

Relative artifact URLs in the manifest stay under that directory. The fetcher already allows `https://ro.turoran.com/...` only (plus `file://` and loopback `http://` for tests).

Serving options (pick one when enabling; do not apply yet):

1. **remoteclient2** document root: place files under the existing `client/` tree (or an overlay path already served for `/client/`).
2. **HAProxy** HTTPS frontend for `ro.turoran.com`: path ACL `/client/releases/` → static dir / backend.

No new public hostname.

### Release pipeline (every merge to `main`)

```
merge → build (Linux + Windows) → sign manifest (offline seed) → publish to main channel
         ↑                              ↑
    gha-elastic                      controller or msi1 host-local signer
    (unsigned artifacts only)        (seed NEVER on GitHub / NEVER in ephemeral VM)
```

1. **Build (CI, unsigned)**  
   Workflow on `push` to `main` with `runs-on: [self-hosted, elastic]` (Turoran gha-elastic; placement prefers **msi1**). Job builds Linux and Windows clients, uploads **unsigned** binaries as Actions artifacts (or to a private staging path). No signing key in the workflow, repo secrets, or ephemeral guest.

2. **Sign (seed off GitHub)** — choose one when enabling:  
   - **Preferred: controller poller / post-merge webhook** on the controller box. Watches `Turor/korangar` main merges (or workflow completion), downloads unsigned artifacts, runs `scripts/sign-korangar-release.sh` / `korangar-sign-release` with `--private-key` pointing at the local seed (e.g. `~/Work/korangar-signing/update.seed`), writes `manifest.jsonup`.  
   - **Alternative: msi1 host-local signer** — a process or non-ephemeral runner on the **msi1 host** that reads the seed from a **host-local path** (mode 600). Do **not** copy the seed into gha-elastic ephemeral guests (VMID 900–999); those VMs are destroyed after the job and must never see key material.

3. **Publish to `main` channel**  
   After a good signature, upload `manifest.jsonup` + both binaries to `/client/releases/main/` on the chosen static backend. Atomic replace of the manifest (write temp + rename) so clients never see a half-written envelope.

4. **Client launch**  
   Updater fetches the `main` channel manifest URL, verifies the embedded Ed25519 public key + per-file SHA256, downloads, atomically swaps the exe (Windows-safe rollback), relaunches. If unset/unreachable, startup continues on the current binary (no hard fail closed for missing channel until Matthew turns the URL on).

### What stays offline

- Ed25519 **private seed** and passphrase: controller `~/Work/korangar-signing/` (or Bitwarden → restored only to the signer host). Never committed, never a GitHub Actions secret, never SCP’d into ephemeral elastic VMs.
- Public key only is embedded in the client (`EMBEDDED_UPDATE_PUBLIC_KEY`).

### Explicit non-goals for this PR

- No live remoteclient2 / HAProxy / DNS change.
- No published `main` manifest or binaries.
- No workflow enabled that publishes or holds the seed.
- Enabling the pipeline is a separate Matthew-approved change.

### Operator cheat-sheet (after approval)

```bash
# on signer host, seed mode 600
./scripts/sign-korangar-release.sh \
  --private-key "$HOME/Work/korangar-signing/update.seed" \
  --version "main-$(git rev-parse --short HEAD)" \
  --out /tmp/manifest.jsonup \
  --file korangar-linux=<path-to-linux-bin> \
  --file korangar-windows.exe=<path-to-windows-exe>
# then publish /tmp/manifest.jsonup + bins to client/releases/main/
```
