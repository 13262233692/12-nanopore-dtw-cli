#!/bin/bash
set -e

echo "=========================================="
echo "  Nanopore DTW CLI - Cross Compilation Script"
echo "=========================================="

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

TARGETS=(
    "x86_64-unknown-linux-gnu"
    "x86_64-unknown-linux-musl"
    "aarch64-unknown-linux-gnu"
    "aarch64-unknown-linux-musl"
    "x86_64-pc-windows-msvc"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
)

BUILD_TYPE=${1:-release}
OUTPUT_DIR="${PROJECT_DIR}/dist"

mkdir -p "$OUTPUT_DIR"

echo ""
echo "Build type: $BUILD_TYPE"
echo "Output directory: $OUTPUT_DIR"
echo ""

for target in "${TARGETS[@]}"; do
    echo ""
    echo "Building for target: $target"
    echo "----------------------------------------"
    
    if [ ! -f "Cargo.toml" ]; then
        echo "Error: Cargo.toml not found in $PROJECT_DIR"
        exit 1
    fi

    if command -v cross &> /dev/null; then
        cross build --target "$target" --profile "$BUILD_TYPE" --features "static simd"
    else
        echo "Warning: cross not found, using cargo"
        cargo build --target "$target" --profile "$BUILD_TYPE" --features "static simd"
    fi

    BIN_NAME="nanopore-dtw"
    if [[ "$target" == *windows* ]]; then
        BIN_NAME="${BIN_NAME}.exe"
    fi

    if [ -f "target/$target/$BUILD_TYPE/$BIN_NAME" ]; then
        DIST_DIR="$OUTPUT_DIR/$target"
        mkdir -p "$DIST_DIR"
        
        cp "target/$target/$BUILD_TYPE/$BIN_NAME" "$DIST_DIR/"
        
        if [[ "$BUILD_TYPE" == "release" ]]; then
            if [[ "$target" != *windows* ]]; then
                if command -v strip &> /dev/null; then
                    strip "$DIST_DIR/$BIN_NAME" 2>/dev/null || true
                fi
            fi
        fi

        SIZE=$(du -h "$DIST_DIR/$BIN_NAME" | cut -f1)
        echo "✓ Built successfully: $DIST_DIR/$BIN_NAME ($SIZE)"
    else
        echo "✗ Build failed for $target"
    fi
done

echo ""
echo "=========================================="
echo "  Build Summary"
echo "=========================================="
echo "Output directory: $OUTPUT_DIR"
for target in "${TARGETS[@]}"; do
    if [ -d "$OUTPUT_DIR/$target" ]; then
        BIN_NAME="nanopore-dtw"
        if [[ "$target" == *windows* ]]; then
            BIN_NAME="${BIN_NAME}.exe"
        fi
        if [ -f "$OUTPUT_DIR/$target/$BIN_NAME" ]; then
            SIZE=$(du -h "$OUTPUT_DIR/$target/$BIN_NAME" | cut -f1)
            echo "  $target: $SIZE"
        fi
    fi
done
echo "=========================================="
