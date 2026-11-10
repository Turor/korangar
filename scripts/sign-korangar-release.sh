#!/usr/bin/env bash
# Wrap `korangar-sign-release sign`. The private seed stays outside the repo.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/sign-korangar-release.sh --version VER --build N --out MANIFEST --file REL=SOURCE [--url URL]...
       [--private-key PATH]

Private key: --private-key PATH, or env KORANGAR_UPDATE_SIGNING_KEY (path to a
32-byte seed, or 64 hex characters). This script never prints the seed.

Binary: $KORANGAR_SIGN_BIN, else `korangar-sign-release` on PATH, else
`cargo run -p korangar-update --bin korangar-sign-release`.
EOF
}

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
private=""
args=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --private-key)
      if [[ $# -lt 2 ]]; then
        echo "missing --private-key value" >&2
        exit 1
      fi
      private="$2"
      shift 2
      ;;
    *)
      args+=("$1")
      shift
      ;;
  esac
done

if [[ -z "$private" && -z "${KORANGAR_UPDATE_SIGNING_KEY:-}" ]]; then
  echo "pass --private-key PATH or set KORANGAR_UPDATE_SIGNING_KEY" >&2
  exit 1
fi

sign_args=(sign)
if [[ -n "$private" ]]; then
  sign_args+=(--private-key "$private")
fi
if [[ ${#args[@]} -eq 0 ]]; then
  usage >&2
  exit 1
fi
sign_args+=("${args[@]}")

if [[ -n "${KORANGAR_SIGN_BIN:-}" ]]; then
  exec "$KORANGAR_SIGN_BIN" "${sign_args[@]}"
fi
if command -v korangar-sign-release >/dev/null 2>&1; then
  exec korangar-sign-release "${sign_args[@]}"
fi

cd "$root"
exec cargo run -p korangar-update --bin korangar-sign-release -- "${sign_args[@]}"
