#!/usr/bin/env bash
set -euo pipefail

COMMAND="${1:-test}"
MODE="${2:-release}"
SKIP_SMOKE="${3:-0}"
MANIFEST="${4:-rust/Cargo.toml}"
MANIFEST_DIR="$(cd "$(dirname "$MANIFEST")" && pwd)"

require_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Missing required tool: $1" >&2
        exit 1
    fi
}

case "$COMMAND" in
    check|build|test) ;;
    *)
        echo "Invalid command '$COMMAND'. Expected check|build|test." >&2
        exit 1
        ;;
esac

case "$MODE" in
    release|debug) ;;
    *)
        echo "Invalid mode '$MODE'. Expected release|debug." >&2
        exit 1
        ;;
esac

if [[ ! -f "$MANIFEST" ]]; then
    echo "Cargo.toml not found at $MANIFEST" >&2
    exit 1
fi

require_tool cargo
require_tool pkg-config
require_tool gdal-config
require_tool geos-config

echo "Using manifest: $MANIFEST"
echo "GDAL: $(gdal-config --version)"
echo "GEOS: $(geos-config --version)"

cargo_args=("$COMMAND" "--manifest-path" "$MANIFEST")
if [[ "$MODE" == "release" ]]; then
    cargo_args+=("--release")
fi

echo "Running cargo $COMMAND ($MODE)..."
cargo "${cargo_args[@]}"

if [[ "$SKIP_SMOKE" != "1" && ( "$COMMAND" == "build" || "$COMMAND" == "test" ) ]]; then
    echo "Running CLI smoke checks ($MODE)..."
    run_args=("run" "--manifest-path" "$MANIFEST")
    if [[ "$MODE" == "release" ]]; then
        run_args+=("--release")
    fi

    cargo "${run_args[@]}" -- --help >/dev/null
    cargo "${run_args[@]}" -- simplify --help >/dev/null
    cargo "${run_args[@]}" -- reduce-bend --help >/dev/null
fi

if [[ "$COMMAND" == "build" || "$COMMAND" == "test" ]]; then
    source_bin="$MANIFEST_DIR/target/$MODE/geo-simplify"
    dist_dir="$MANIFEST_DIR/dist"
    dest_bin="$dist_dir/geo-simplify-linux-x64"

    if [[ ! -f "$source_bin" ]]; then
        echo "Built binary not found at $source_bin" >&2
        exit 1
    fi

    mkdir -p "$dist_dir"
    cp "$source_bin" "$dest_bin"
    chmod +x "$dest_bin"

    echo "Copied Linux x64 binary to: $dest_bin"
fi
