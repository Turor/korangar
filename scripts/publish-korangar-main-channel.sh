#!/usr/bin/env bash
# Upload a signed main-channel release to Garage bucket korangar-releases.
# Run on the controller. Never prints key material.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/publish-korangar-main-channel.sh --sha GITSHA --manifest PATH \
         --linux PATH --windows PATH

Uploads (atomically for the manifest) to s3://korangar-releases/main/ and
keeps the last 3 git SHAs. Requires:
  /home/controller/Work/korangar-signing/garage-writer.env
  (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_DEFAULT_REGION=garage)
Jump host: SSH_EXTRA='-J root@192.168.11.3' GARAGE_HOST=root@192.168.11.20
EOF
}

SHA=""
MANIFEST=""
LINUX=""
WINDOWS=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --sha) SHA="${2:-}"; shift 2 ;;
    --manifest) MANIFEST="${2:-}"; shift 2 ;;
    --linux) LINUX="${2:-}"; shift 2 ;;
    --windows) WINDOWS="${2:-}"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; usage >&2; exit 1 ;;
  esac
done

[[ -n "$SHA" && -f "$MANIFEST" && -f "$LINUX" && -f "$WINDOWS" ]] || {
  usage >&2
  exit 1
}

CRED="${KORANGAR_GARAGE_ENV:-$HOME/Work/korangar-signing/garage-writer.env}"
[[ -f "$CRED" ]] || { echo "missing garage writer env: $CRED" >&2; exit 1; }
GARAGE_HOST="${GARAGE_HOST:-root@192.168.11.20}"
SSH_EXTRA="${SSH_EXTRA:--J root@192.168.11.3}"
ENDPOINT="${ENDPOINT:-http://127.0.0.1:3900}"
BUCKET="${BUCKET:-korangar-releases}"
PREFIX="main"
LINUX_KEY="${PREFIX}/korangar-linux-${SHA}"
WIN_KEY="${PREFIX}/korangar-windows-${SHA}.exe"
MANIFEST_KEY="${PREFIX}/manifest.jsonup"
STAGING_KEY="${PREFIX}/.manifest.jsonup.${SHA}.tmp"

WORKDIR=$(mktemp -d)
trap 'rm -rf "$WORKDIR"' EXIT
chmod 700 "$WORKDIR"
cp "$MANIFEST" "$WORKDIR/manifest.jsonup"
cp "$LINUX" "$WORKDIR/linux.bin"
cp "$WINDOWS" "$WORKDIR/windows.exe"
# credentials stay mode 600 and are sourced only on garage-1
ssh -o BatchMode=yes $SSH_EXTRA "$GARAGE_HOST" -- mkdir -p /root/korangar-release-upload && chmod 700 /root/korangar-release-upload
scp -q -o BatchMode=yes $SSH_EXTRA "$CRED" \
  "$WORKDIR/manifest.jsonup" "$WORKDIR/linux.bin" "$WORKDIR/windows.exe" \
  "${GARAGE_HOST}:/root/korangar-release-upload/"

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
put "${LINUX_KEY}" linux.bin
put "${WIN_KEY}" windows.exe
put "${STAGING_KEY}" manifest.jsonup
# Atomic-ish replace: copy staging over the live key, then delete staging.
aws --endpoint-url ${ENDPOINT} s3api copy-object --bucket ${BUCKET} \
  --copy-source "${BUCKET}/${STAGING_KEY}" --key "${MANIFEST_KEY}" >/dev/null
aws --endpoint-url ${ENDPOINT} s3api delete-object --bucket ${BUCKET} --key "${STAGING_KEY}" >/dev/null

# Keep last 3 SHAs (plus the one just published).
mapfile -t keys < <(aws --endpoint-url ${ENDPOINT} s3api list-objects-v2 --bucket ${BUCKET} --prefix "${PREFIX}/korangar-linux-" --query 'Contents[].Key' --output text | tr '\t' '\n' | sed '/^$/d' | sort)
# Extract shas and unique, drop current, keep oldest excess.
declare -A seen=()
shas=()
for k in "\${keys[@]}"; do
  s=\${k##*-}
  if [[ -z "\${seen[\$s]:-}" ]]; then
    seen[\$s]=1
    shas+=("\$s")
  fi
done
# list-objects is lexical; publish time is better but lexical sha is unstable.
# Use LastModified via a second query.
mapfile -t ordered < <(aws --endpoint-url ${ENDPOINT} s3api list-objects-v2 --bucket ${BUCKET} --prefix "${PREFIX}/korangar-linux-" --query 'reverse(sort_by(Contents,&LastModified))[].Key' --output text | tr '\t' '\n' | sed '/^$/d')
keep=()
for k in "\${ordered[@]}"; do
  s=\${k##*-}
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
# delete linux/windows objects whose sha is not in keep
mapfile -t all < <(aws --endpoint-url ${ENDPOINT} s3api list-objects-v2 --bucket ${BUCKET} --prefix "${PREFIX}/" --query 'Contents[].Key' --output text | tr '\t' '\n' | sed '/^$/d')
for k in "\${all[@]}"; do
  case "\$k" in
    ${PREFIX}/manifest.jsonup|${PREFIX}/.manifest.jsonup.*) continue ;;
    ${PREFIX}/korangar-linux-*|${PREFIX}/korangar-windows-*.exe)
      s=\$k
      s=\${s##*-}
      s=\${s%.exe}
      drop=1
      for x in "\${keep[@]}"; do
        [[ "\$x" == "\$s" ]] && drop=0
      done
      if [[ \$drop -eq 1 ]]; then
        aws --endpoint-url ${ENDPOINT} s3api delete-object --bucket ${BUCKET} --key "\$k" >/dev/null
      fi
      ;;
  esac
done
rm -f /root/korangar-release-upload/$(basename "$CRED") linux.bin windows.exe manifest.jsonup
echo "published ${MANIFEST_KEY} sha=${SHA}"
EOF
