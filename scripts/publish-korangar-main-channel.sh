#!/usr/bin/env bash
# Upload a signed main-channel release to Garage bucket korangar-releases.
# Run on the controller. Never prints key material.
# Linux and Windows objects are each optional. At least one is required.
# --dry-run prints the manifest file list and the object keys, and does not upload.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/publish-korangar-main-channel.sh --sha GITSHA --manifest PATH \
         [--linux PATH] [--windows PATH] [--dry-run]

Either --linux or --windows is required. The manifest must list that
platform's release binary and must not list a platform that is not being
uploaded. An empty manifest (no file entries) is refused.

--dry-run prints the manifest paths and the object keys that would be
uploaded. It does not read Garage credentials or open a connection.

Uploads (atomically for the manifest) to s3://korangar-releases/main/ and
keeps the last 3 git SHAs across both platforms. Requires (not for --dry-run):
  /home/controller/Work/korangar-signing/garage-writer.env
  (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_DEFAULT_REGION=garage)
Jump host: SSH_EXTRA='-J root@192.168.11.3' GARAGE_HOST=root@192.168.11.20
EOF
}

# Print one relative path per line from a KRUP1 or KRUP1SIG document.
list_manifest_files() {
  awk '
    $0 == "KRUP1" { state = 1; next }
    state == 1 { state = 2; next }
    state == 2 { state = 3; next }
    state == 3 { left = $0 + 0; state = 4; next }
    state == 4 && left > 0 { print; state = 5; next }
    state == 5 { state = 6; next }
    state == 6 { state = 7; next }
    state == 7 { left--; state = 4; next }
  ' "$1"
}

SHA=""
MANIFEST=""
LINUX=""
WINDOWS=""
DRY=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --dry-run) DRY=1; shift ;;
    --sha) SHA="${2:-}"; shift 2 ;;
    --manifest) MANIFEST="${2:-}"; shift 2 ;;
    --linux) LINUX="${2:-}"; shift 2 ;;
    --windows) WINDOWS="${2:-}"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; usage >&2; exit 1 ;;
  esac
done

if [[ -z "$SHA" || -z "$MANIFEST" || ! -f "$MANIFEST" ]]; then
  usage >&2
  exit 1
fi
if [[ "$SHA" == *"/"* || "$SHA" == *" "* || "$SHA" == *"."* ]]; then
  echo "refusing unsafe --sha" >&2
  exit 1
fi
if [[ -z "$LINUX" && -z "$WINDOWS" ]]; then
  echo "refusing an empty manifest: pass --linux and/or --windows" >&2
  exit 1
fi
if [[ -n "$LINUX" && ! -f "$LINUX" ]]; then
  echo "linux artifact is not a file: $LINUX" >&2
  exit 1
fi
if [[ -n "$WINDOWS" && ! -f "$WINDOWS" ]]; then
  echo "windows artifact is not a file: $WINDOWS" >&2
  exit 1
fi

mapfile -t MANIFEST_FILES < <(list_manifest_files "$MANIFEST")
if [[ ${#MANIFEST_FILES[@]} -eq 0 ]]; then
  echo "refusing an empty manifest: ${MANIFEST} has no file entries" >&2
  exit 1
fi

LINUX_NAME="korangar-linux-${SHA}"
WIN_NAME="korangar-windows-${SHA}.exe"
has_named() {
  local name="$1"
  local entry
  for entry in "${MANIFEST_FILES[@]}"; do
    [[ "$entry" == "$name" ]] && return 0
  done
  return 1
}
if [[ -n "$LINUX" ]] && ! has_named "$LINUX_NAME"; then
  echo "manifest has no ${LINUX_NAME}" >&2
  exit 1
fi
if [[ -n "$WINDOWS" ]] && ! has_named "$WIN_NAME"; then
  echo "manifest has no ${WIN_NAME}" >&2
  exit 1
fi
if [[ -z "$LINUX" ]] && has_named "$LINUX_NAME"; then
  echo "manifest lists ${LINUX_NAME} but --linux was not passed" >&2
  exit 1
fi
if [[ -z "$WINDOWS" ]] && has_named "$WIN_NAME"; then
  echo "manifest lists ${WIN_NAME} but --windows was not passed" >&2
  exit 1
fi

PREFIX="main"
LINUX_KEY="${PREFIX}/${LINUX_NAME}"
WIN_KEY="${PREFIX}/${WIN_NAME}"
MANIFEST_KEY="${PREFIX}/manifest.jsonup"

if [[ "$DRY" -eq 1 ]]; then
  echo "dry-run"
  echo "sha: ${SHA}"
  echo "manifest: ${MANIFEST}"
  echo "files:"
  for entry in "${MANIFEST_FILES[@]}"; do
    printf '%s\n' "$entry"
  done
  echo "objects:"
  if [[ -n "$LINUX" ]]; then
    printf '%s\n' "$LINUX_KEY"
  fi
  if [[ -n "$WINDOWS" ]]; then
    printf '%s\n' "$WIN_KEY"
  fi
  printf '%s\n' "$MANIFEST_KEY"
  exit 0
fi

CRED="${KORANGAR_GARAGE_ENV:-$HOME/Work/korangar-signing/garage-writer.env}"
[[ -f "$CRED" ]] || { echo "missing garage writer env: $CRED" >&2; exit 1; }
GARAGE_HOST="${GARAGE_HOST:-root@192.168.11.20}"
SSH_EXTRA="${SSH_EXTRA:--J root@192.168.11.3}"
ENDPOINT="${ENDPOINT:-http://127.0.0.1:3900}"
BUCKET="${BUCKET:-korangar-releases}"
STAGING_KEY="${PREFIX}/.manifest.jsonup.${SHA}.tmp"

WORKDIR=$(mktemp -d)
trap 'rm -rf "$WORKDIR"' EXIT
chmod 700 "$WORKDIR"
cp "$MANIFEST" "$WORKDIR/manifest.jsonup"
SCP_FILES=("$CRED" "$WORKDIR/manifest.jsonup")
if [[ -n "$LINUX" ]]; then
  cp "$LINUX" "$WORKDIR/linux.bin"
  SCP_FILES+=("$WORKDIR/linux.bin")
fi
if [[ -n "$WINDOWS" ]]; then
  cp "$WINDOWS" "$WORKDIR/windows.exe"
  SCP_FILES+=("$WORKDIR/windows.exe")
fi
HAS_LINUX=$([[ -n "$LINUX" ]] && echo 1 || echo 0)
HAS_WINDOWS=$([[ -n "$WINDOWS" ]] && echo 1 || echo 0)
# credentials stay mode 600 and are sourced only on garage-1
ssh -o BatchMode=yes $SSH_EXTRA "$GARAGE_HOST" -- "mkdir -p /root/korangar-release-upload && chmod 700 /root/korangar-release-upload"
scp -q -o BatchMode=yes $SSH_EXTRA "${SCP_FILES[@]}" "${GARAGE_HOST}:/root/korangar-release-upload/"

ssh -o BatchMode=yes $SSH_EXTRA "$GARAGE_HOST" -- bash -s <<EOF
set -euo pipefail
cd /root/korangar-release-upload
set -a
# shellcheck disable=SC1091
source ./$(basename "$CRED")
set +a
export AWS_DEFAULT_REGION="\${AWS_DEFAULT_REGION:-garage}"
export AWS_REQUEST_CHECKSUM_CALCULATION=when_required
export AWS_EC2_METADATA_DISABLED=true
put() {
  local key="\$1" file="\$2"
  aws --endpoint-url ${ENDPOINT} s3api put-object --bucket ${BUCKET} --key "\$key" --body "\$file" >/dev/null
}
if [[ ${HAS_LINUX} == 1 ]]; then put "${LINUX_KEY}" linux.bin; fi
if [[ ${HAS_WINDOWS} == 1 ]]; then put "${WIN_KEY}" windows.exe; fi
put "${STAGING_KEY}" manifest.jsonup
# Atomic-ish replace: copy staging over the live key, then delete staging.
aws --endpoint-url ${ENDPOINT} s3api copy-object --bucket ${BUCKET} \
  --copy-source "${BUCKET}/${STAGING_KEY}" --key "${MANIFEST_KEY}" >/dev/null
aws --endpoint-url ${ENDPOINT} s3api delete-object --bucket ${BUCKET} --key "${STAGING_KEY}" >/dev/null

# Keep the last 3 SHAs across both platforms. A Windows-only object must count.
mapfile -t ordered < <(aws --endpoint-url ${ENDPOINT} s3api list-objects-v2 --bucket ${BUCKET} --prefix "${PREFIX}/" --query 'reverse(sort_by(Contents,&LastModified))[].Key' --output text | tr '\t' '\n' | sed '/^$/d')
keep=()
for k in "\${ordered[@]}"; do
  case "\$k" in
    ${PREFIX}/korangar-linux-*|${PREFIX}/korangar-windows-*.exe) ;;
    *) continue ;;
  esac
  s=\${k##*-}
  s=\${s%.exe}
  already=0
  for x in "\${keep[@]+"\${keep[@]}"}"; do
    [[ "\$x" == "\$s" ]] && already=1
  done
  if [[ \$already -eq 0 ]]; then
    keep+=("\$s")
  fi
  if [[ \${#keep[@]} -ge 3 ]]; then
    break
  fi
done
mapfile -t all < <(aws --endpoint-url ${ENDPOINT} s3api list-objects-v2 --bucket ${BUCKET} --prefix "${PREFIX}/" --query 'Contents[].Key' --output text | tr '\t' '\n' | sed '/^$/d')
for k in "\${all[@]}"; do
  case "\$k" in
    ${PREFIX}/manifest.jsonup|${PREFIX}/.manifest.jsonup.*) continue ;;
    ${PREFIX}/korangar-linux-*|${PREFIX}/korangar-windows-*.exe)
      s=\$k
      s=\${s##*-}
      s=\${s%.exe}
      drop=1
      for x in "\${keep[@]+"\${keep[@]}"}"; do
        [[ "\$x" == "\$s" ]] && drop=0
      done
      if [[ \$drop -eq 1 ]]; then
        aws --endpoint-url ${ENDPOINT} s3api delete-object --bucket ${BUCKET} --key "\$k" >/dev/null
      fi
      ;;
  esac
done
rm -f /root/korangar-release-upload/$(basename "$CRED") linux.bin windows.exe manifest.jsonup
echo "published ${MANIFEST_KEY} sha=${SHA} linux=${HAS_LINUX} windows=${HAS_WINDOWS}"
EOF
