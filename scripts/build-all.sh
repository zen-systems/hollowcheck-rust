#!/bin/bash
# Build hollowcheck for all supported platforms
# Requires: cargo, cross (cargo install cross)

set -e

TARGETS=(
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu"
    "x86_64-pc-windows-gnu"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
)

OUTPUT_DIR="dist"
mkdir -p "$OUTPUT_DIR"

echo "Building hollowcheck for all platforms..."

for target in "${TARGETS[@]}"; do
    echo ""
    echo "=== Building for $target ==="

    # Determine output name
    case "$target" in
        *linux*x86_64*)
            output_name="hollowcheck-linux-amd64"
            ;;
        *linux*aarch64*)
            output_name="hollowcheck-linux-arm64"
            ;;
        *windows*)
            output_name="hollowcheck-windows-amd64.exe"
            ;;
        *darwin*x86_64*)
            output_name="hollowcheck-darwin-amd64"
            ;;
        *darwin*aarch64*)
            output_name="hollowcheck-darwin-arm64"
            ;;
    esac

    # Use cross for Linux ARM and Windows, cargo for others
    case "$target" in
        aarch64-unknown-linux-gnu|x86_64-pc-windows-gnu)
            if command -v cross &> /dev/null; then
                cross build --release --target "$target"
            else
                echo "Skipping $target (cross not installed)"
                continue
            fi
            ;;
        *darwin*)
            # macOS targets need to be built on macOS
            if [[ "$(uname)" == "Darwin" ]]; then
                cargo build --release --target "$target"
            else
                echo "Skipping $target (requires macOS)"
                continue
            fi
            ;;
        *)
            cargo build --release --target "$target"
            ;;
    esac

    # Copy binary to output directory
    if [[ "$target" == *windows* ]]; then
        cp "target/$target/release/hollowcheck.exe" "$OUTPUT_DIR/$output_name"
    else
        cp "target/$target/release/hollowcheck" "$OUTPUT_DIR/$output_name"
    fi

    echo "Built: $OUTPUT_DIR/$output_name"
done

echo ""
echo "=== Build Summary ==="
ls -la "$OUTPUT_DIR/"

# Generate checksums
echo ""
echo "=== Generating checksums ==="
cd "$OUTPUT_DIR"
sha256sum hollowcheck-* > checksums.txt
cat checksums.txt
