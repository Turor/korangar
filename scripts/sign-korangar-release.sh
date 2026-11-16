#!/usr/bin/env bash
# Wrap `korangar-sign-release sign`. The private seed stays outside the repo.
# Linux and Windows release binaries are each optional. At least one is required.
# --dry-run prints the manifest file list and the key path, and does not sign.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/sign-korangar-release.sh --version VER --build N --out MANIFEST \
       [--sha GITSHA] [--linux PATH] [--windows PATH] \
       [--file REL=SOURCE [--url URL]]... [--private-key PATH] [--dry-run]

Either --linux or --windows (or a --file named korangar-linux-* /
korangar-windows-*.exe) is required. Passing neither refuses an empty
manifest. --linux and --windows need --sha and become
korangar-linux-<sha> and korangar-windows-<sha>.exe.

--dry-run prints the file list and the key path only. It does not read the
seed, sign, or write the manifest.

Private key: --private-key PATH, or env KORANGAR_UPDATE_SIGNING_KEY (path to a
32-byte seed, or 64 hex characters). This script never prints the seed.

Binary: $KORANGAR_SIGN_BIN, else `korangar-sign-release` on PATH, else
`cargo run -p korangar-update --bin korangar-sign-release`.
EOF
}

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
private=""
dry=0
sha=""
linux_path=""
windows_path=""
files=()
urls=()
forward=()

add_file() {
  files+=("$1")
  urls+=("")
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --dry-run)
      dry=1
      shift
      ;;
    --private-key)
      if [[ $# -lt 2 ]]; then
        echo "missing --private-key value" >&2
        exit 1
      fi
      private="$2"
      shift 2
      ;;
    --sha)
      if [[ $# -lt 2 ]]; then
        echo "missing --sha value" >&2
        exit 1
      fi
      sha="$2"
      shift 2
      ;;
    --linux)
      if [[ $# -lt 2 ]]; then
        echo "missing --linux value" >&2
        exit 1
      fi
      linux_path="$2"
      shift 2
      ;;
    --windows)
      if [[ $# -lt 2 ]]; then
        echo "missing --windows value" >&2
        exit 1
      fi
      windows_path="$2"
      shift 2
      ;;
    --version|--build|--out)
      if [[ $# -lt 2 ]]; then
        echo "missing $1 value" >&2
        exit 1
      fi
      forward+=("$1" "$2")
      shift 2
      ;;
    --file)
      if [[ $# -lt 2 ]]; then
        echo "missing --file value" >&2
        exit 1
      fi
      add_file "$2"
      shift 2
      ;;
    --url)
      if [[ $# -lt 2 || ${#files[@]} -eq 0 ]]; then
        echo "--url needs a preceding --file" >&2
        exit 1
      fi
      urls[$((${#files[@]} - 1))]="$2"
      shift 2
      ;;
    *)
      echo "unknown arg: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -n "$linux_path" || -n "$windows_path" ]]; then
  if [[ -z "$sha" ]]; then
    echo "--linux and --windows require --sha" >&2
    exit 1
  fi
  if [[ "$sha" == *"/"* || "$sha" == *" "* || "$sha" == *"."* ]]; then
    echo "refusing unsafe --sha" >&2
    exit 1
  fi
fi
if [[ -n "$linux_path" ]]; then
  add_file "korangar-linux-${sha}=${linux_path}"
fi
if [[ -n "$windows_path" ]]; then
  add_file "korangar-windows-${sha}.exe=${windows_path}"
fi

if [[ ${#files[@]} -eq 0 ]]; then
  echo "refusing an empty manifest: pass --linux, --windows, or --file" >&2
  exit 1
fi

has_platform=0
for spec in "${files[@]}"; do
  rel="${spec%%=*}"
  if [[ "$rel" == korangar-linux-* || "$rel" == korangar-windows-*.exe ]]; then
    has_platform=1
  fi
done
if [[ "$has_platform" -eq 0 ]]; then
  echo "refusing manifest with neither a linux nor a windows build" >&2
  exit 1
fi

key_display="(not set)"
if [[ -n "$private" ]]; then
  key_display="$private"
elif [[ -n "${KORANGAR_UPDATE_SIGNING_KEY:-}" ]]; then
  if [[ "${KORANGAR_UPDATE_SIGNING_KEY}" =~ ^[0-9A-Fa-f]{64}$ ]]; then
    key_display="(env seed, not printed)"
  else
    key_display="${KORANGAR_UPDATE_SIGNING_KEY}"
  fi
fi

if [[ "$dry" -eq 1 ]]; then
  echo "dry-run"
  echo "key: ${key_display}"
  echo "files:"
  for spec in "${files[@]}"; do
    printf '%s\n' "${spec%%=*}"
  done
  exit 0
fi

if [[ -z "$private" && -z "${KORANGAR_UPDATE_SIGNING_KEY:-}" ]]; then
  echo "pass --private-key PATH or set KORANGAR_UPDATE_SIGNING_KEY" >&2
  exit 1
fi

sign_args=(sign)
if [[ -n "$private" ]]; then
  sign_args+=(--private-key "$private")
fi
sign_args+=("${forward[@]}")
for index in "${!files[@]}"; do
  sign_args+=(--file "${files[$index]}")
  if [[ -n "${urls[$index]}" ]]; then
    sign_args+=(--url "${urls[$index]}")
  fi
done

if [[ -n "${KORANGAR_SIGN_BIN:-}" ]]; then
  exec "$KORANGAR_SIGN_BIN" "${sign_args[@]}"
fi
if command -v korangar-sign-release >/dev/null 2>&1; then
  exec korangar-sign-release "${sign_args[@]}"
fi

cd "$root"
exec cargo run -p korangar-update --bin korangar-sign-release -- "${sign_args[@]}"
