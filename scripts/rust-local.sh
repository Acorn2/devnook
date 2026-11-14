#!/bin/sh
# Prefer the developer's Rust; use the optional project-local toolchain when present.
set -eu
devnook_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
if command -v cargo >/dev/null 2>&1; then
  exec "$@"
fi
if [ ! -x "$devnook_root/.tmp/toolchain/cargo/bin/cargo" ]; then
  echo "Rust not found. Install stable Rust before building DevNook." >&2
  exit 1
fi
export RUSTUP_HOME="$devnook_root/.tmp/toolchain/rustup"
export CARGO_HOME="$devnook_root/.tmp/toolchain/cargo"
export PATH="$CARGO_HOME/bin:$PATH"
exec "$@"
