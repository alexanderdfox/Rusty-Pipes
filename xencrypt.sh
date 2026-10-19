#!/usr/bin/env bash
set -euo pipefail

XPPIPE_BIN="./target/release/xpipe"
ACTION="${1:-}"
INPUT_FILE="${2:-}"
OUTPUT_FILE="${3:-}"
KEY_FILE="${4:-}"

usage() {
    echo "Usage: $0 [encrypt|decrypt] <input_file> <output_file> <key_file>"
    exit 1
}

if [[ $# -lt 4 ]]; then
    usage
fi

if [[ ! -x "$XPPIPE_BIN" ]]; then
    echo "Error: xpipe binary not found at $XPPIPE_BIN. Run 'cargo build --release' first."
    exit 1
fi

case "$ACTION" in
    encrypt|decrypt)
        echo "[*] Starting $ACTION process..."
        
        if [[ "$ACTION" == "encrypt" && ! -f "$KEY_FILE" ]]; then
            echo "[*] Generating random keystream..."
            KEY_SIZE=$(wc -c < "$INPUT_FILE" | tr -d ' ')
            head -c "$KEY_SIZE" /dev/urandom > "$KEY_FILE"
        fi

        if [[ ! -f "$KEY_FILE" ]]; then
            echo "Error: Key file '$KEY_FILE' not found."
            exit 1
        fi

        "$XPPIPE_BIN" \
            --west "$INPUT_FILE" \
            --north "$KEY_FILE" \
            --east "$OUTPUT_FILE" \
            --buffer-size 5000

        echo "[+] Success! Output written to $OUTPUT_FILE"
        ;;
    *)
        usage
        ;;
esac
