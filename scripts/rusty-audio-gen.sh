#!/usr/bin/env bash
# rusty-audio-gen.sh
# Uses diode + transistor + rustypipes + xpipe to generate shaped 8-bit PCM audio
#
# Usage:
#   ./rusty-audio-gen.sh [seconds] [freq_hz] [gain] [mask_hex] [play|file]
#
# Examples:
#   ./rusty-audio-gen.sh 3 440 160 0x5A play
#   ./rusty-audio-gen.sh 5 220 200 0xA5 file   # writes rusty-tone.raw

set -euo pipefail

# ---------- configuration ----------
DURATION=${1:-3}          # seconds
FREQ=${2:-440}            # Hz
GAIN=${3:-160}            # transistor amplify base (128 = unity)
MASK=${4:-0x5A}           # rustypipes cooling-fin mask
MODE=${5:-play}           # "play" or "file"
SAMPLE_RATE=8000          # keep it modest so pure-bash generation is fast
SAMPLES=$((DURATION * SAMPLE_RATE))

# Locate the binaries (prefer release build)
BIN_DIR="./target/release"
for tool in diode transistor rustypipes xpipe; do
    if [[ ! -x "$BIN_DIR/$tool" ]]; then
        echo "Error: $BIN_DIR/$tool not found. Run 'cargo build --release' first." >&2
        exit 1
    fi
done

# Temporary files
KEYSTREAM=$(mktemp)
OUT_RAW=$(mktemp)
trap 'rm -f "$KEYSTREAM" "$OUT_RAW"' EXIT

echo "[*] Generating $DURATION s of circuit-shaped audio @ ${FREQ} Hz" >&2
echo "    gain=$GAIN  mask=$MASK  sample_rate=$SAMPLE_RATE" >&2

# ---------- 1. Generate a pure sine as floating-point samples ----------
# (awk is available everywhere and fast enough for a few seconds of 8 kHz)
awk -v n="$SAMPLES" -v f="$FREQ" -v sr="$SAMPLE_RATE" '
BEGIN {
    PI = 3.141592653589793
    for (i = 0; i < n; i++) {
        t = i / sr
        # amplitude in [-1.0 .. 1.0]
        print sin(2 * PI * f * t)
    }
}' | \

# ---------- 2. Diode circuit model (nonlinear shaping) ----------
# Mode 1 = feedback diode model  → soft clipping / transfer-curve character
"$BIN_DIR/diode" --mode 1 | \

# ---------- 3. Convert shaped floats → 8-bit unsigned PCM (center 128) ----------
awk '
{
    # clamp to roughly [-1 .. 1] after diode, then map to 0..255
    v = $1
    if (v > 1.0)  v = 1.0
    if (v < -1.0) v = -1.0
    byte = int( (v + 1.0) * 127.5 + 0.5 )
    if (byte < 0)   byte = 0
    if (byte > 255) byte = 255
    printf "%c", byte
}' | \

# ---------- 4. BJT-style gain stage (transistor amplify) ----------
"$BIN_DIR/transistor" -m amplify -b "$GAIN" | \

# ---------- 5. Toroidal pipe + cooling-fin XOR mask ----------
"$BIN_DIR/rustypipes" --mask "$MASK" --mode standard | \

# ---------- 6. Generate a matching keystream and cross-pipe XOR ----------
# (gives the audio an extra “encrypted / textured” layer)
# We generate the keystream on the fly so length always matches
head -c "$SAMPLES" /dev/urandom > "$KEYSTREAM"

# Feed the masked audio as West, keystream as North
"$BIN_DIR/xpipe" \
    --west  - \
    --north "$KEYSTREAM" \
    --east  "$OUT_RAW" \
    --south /dev/null

echo "[+] Pipeline finished. Output size: $(wc -c < "$OUT_RAW") bytes" >&2

# ---------- playback or save ----------
if [[ "$MODE" == "play" ]]; then
    if command -v aplay >/dev/null 2>&1; then
        echo "[*] Playing with aplay (u8 mono ${SAMPLE_RATE} Hz)…" >&2
        aplay -f U8 -r "$SAMPLE_RATE" -c 1 "$OUT_RAW"
    elif command -v ffplay >/dev/null 2>&1; then
        echo "[*] Playing with ffplay…" >&2
        ffplay -f u8 -ar "$SAMPLE_RATE" -ac 1 -autoexit -nodisp "$OUT_RAW"
    else
        echo "No aplay or ffplay found. Saving to rusty-tone.raw instead." >&2
        cp "$OUT_RAW" rusty-tone.raw
        echo "Play later with: aplay -f U8 -r $SAMPLE_RATE -c 1 rusty-tone.raw" >&2
    fi
else
    cp "$OUT_RAW" rusty-tone.raw
    echo "[+] Saved: rusty-tone.raw" >&2
    echo "    aplay -f U8 -r $SAMPLE_RATE -c 1 rusty-tone.raw" >&2
fi
