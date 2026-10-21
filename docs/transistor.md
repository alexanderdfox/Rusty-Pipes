# transistor — Elaborate Usage Guide

A Linux-style byte-stream filter that behaves like a bipolar junction transistor.

```
Collector  ←  stdin   (data current)
Base       ←  CLI     (control signal)
Emitter    →  stdout  (output current)
```

When the base is high the transistor is conducting and data flows from stdin to stdout.  
When the base is low the transistor is in cutoff and the data stream is discarded.

---

## Table of Contents

1. [Quick Start](#1-quick-start)
2. [Command-Line Reference](#2-command-line-reference)
3. [Switch Mode (Digital)](#3-switch-mode-digital)
4. [Amplify Mode (Analog-ish)](#4-amplify-mode-analog-ish)
5. [Pipeline Patterns](#5-pipeline-patterns)
6. [Scripting & Automation](#6-scripting--automation)
7. [Safety & Interlock Patterns](#7-safety--interlock-patterns)
8. [Creative / Artistic Uses](#8-creative--artistic-uses)
9. [Performance Notes](#9-performance-notes)
10. [Edge Cases & Gotchas](#10-edge-cases--gotchas)
11. [Comparison with Classic Unix Tools](#11-comparison-with-classic-unix-tools)
12. [Extending the Metaphor](#12-extending-the-metaphor)

---

## 1. Quick Start

```bash
# Build
cargo build --release
cp target/release/transistor ~/bin/   # or anywhere on $PATH

# Conducting (data flows)
echo "hello" | transistor --base 1
# → hello

# Cutoff (data is discarded)
echo "hello" | transistor --base 0
# → (nothing)

# Amplify
printf '\x40\x80\xc0' | transistor -m amplify -b 192 | xxd
# → 60 c0 ff
```

---

## 2. Command-Line Reference

```
transistor [OPTIONS]
```

| Flag | Long form | Values | Default | Meaning |
|------|-----------|--------|---------|---------|
| `-m` | `--mode` | `switch` \| `amplify` | `switch` | Operating regime |
| `-b` | `--base` | 0–255 | `1` | Base control value |
| `-t` | `--threshold` | 0–255 | `1` | Level that counts as “high” in switch mode |
| `-h` | `--help` | — | — | Show help and exit |

Exit status is always 0 unless an argument is malformed (then 1).

The program never writes diagnostic messages to stdout; all help and errors go to stderr so they never pollute a data pipeline.

---

## 3. Switch Mode (Digital)

This is the classic “transistor as switch” behaviour.

- `base >= threshold` → **conducting** (saturation) — every byte is copied from stdin to stdout.
- `base <  threshold` → **cutoff** — every byte is read and discarded; stdout stays empty.

### 3.1 Basic on/off

```bash
# Always on
cat data.bin | transistor --base 1 > out.bin

# Always off
cat data.bin | transistor --base 0 > /dev/null
```

### 3.2 Custom threshold

Useful when the control value is not a simple 0/1 flag:

```bash
# Treat any base ≥ 128 as “on”
sensor_stream | transistor -t 128 -b "$SENSOR_LEVEL" | processor
```

### 3.3 Environment-driven switch

```bash
export ENABLE=1
dangerous_generator | transistor -b "$ENABLE" | consumer
```

### 3.4 File-based interlock

```bash
# Only pass data when the arming file exists
generator | transistor -b $(test -f /tmp/armed && echo 1 || echo 0) | actuator
```

### 3.5 Zero-byte streams

An empty stdin is handled correctly in both states:

```bash
: | transistor --base 1   # produces nothing (correct)
: | transistor --base 0   # produces nothing (correct)
```

---

## 4. Amplify Mode (Analog-ish)

Each input byte is treated as an unsigned 8-bit sample and multiplied by a gain factor derived from the base:

```
gain = base / 128.0
```

| base | gain | Effect |
|------|------|--------|
| 0    | 0.0  | Everything becomes 0 |
| 64   | 0.5  | Attenuate by half |
| 128  | 1.0  | Unity gain (buffer) |
| 192  | 1.5  | Boost by 50 % |
| 255  | ≈2.0 | Almost double, then clamp |

Results are rounded and clamped to the range 0–255.

### 4.1 Brighten a raw image or audio stream

```bash
cat image.gray | transistor -m amplify -b 180 > brighter.gray
```

### 4.2 Soft attenuation

```bash
cat loud.u8 | transistor -m amplify -b 90 > quieter.u8
```

### 4.3 Hard clip toward white

```bash
cat samples.u8 | transistor -m amplify -b 255 > clipped.u8
```

### 4.4 Variable gain from a control file

```bash
GAIN=$(cat /tmp/gain)          # 0–255
cat stream | transistor -m amplify -b "$GAIN" > out
```

### 4.5 Chained gain stages

```bash
# First stage mild boost, second stage another mild boost
cat data | transistor -m amplify -b 160 | transistor -m amplify -b 160 > out
```

---

## 5. Pipeline Patterns

### 5.1 Classic three-stage pipe

```bash
producer | transistor -b "$CONTROL" | consumer
```

### 5.2 Fan-out with tee (observe both sides)

```bash
producer | tee /tmp/raw.bin | transistor -b 1 | tee /tmp/passed.bin | consumer
```

### 5.3 Conditional processing branch

```bash
if [ "$MODE" = "live" ]; then
    BASE=1
else
    BASE=0
fi
sensor | transistor -b "$BASE" | logger
```

### 5.4 Multi-transistor cascade (AND logic)

```bash
# Data only passes if BOTH controls are high
data | transistor -b "$A" | transistor -b "$B" | sink
```

### 5.5 Multi-transistor cascade (OR-like via parallel)

```bash
# Rough OR: run two transistors and merge (requires more tooling)
data | tee >(transistor -b "$A" > /tmp/a) >(transistor -b "$B" > /tmp/b)
# then combine /tmp/a and /tmp/b with your own logic
```

### 5.6 Rate-limited or gated logging

```bash
journalctl -f | transistor -b "$(cat /tmp/log_enable)" | logger -t gated
```

---

## 6. Scripting & Automation

### 6.1 Bash function wrapper

```bash
transistor_on()  { transistor --base 1; }
transistor_off() { transistor --base 0; }

# usage
generator | transistor_on | consumer
```

### 6.2 Dynamic base from a sensor

```bash
while true; do
    LEVEL=$(cat /sys/class/hwmon/hwmon0/temp1_input)
    # map temperature to 0–255 or 0/1 as needed
    BASE=$(( LEVEL > 50000 ? 1 : 0 ))
    sensor_data | transistor -b "$BASE" | actuator
    sleep 1
done
```

### 6.3 systemd / cron friendly

Because the program is a pure filter and writes nothing to stdout except data, it is safe inside systemd units and cron jobs:

```bash
# /etc/cron.d/gated-backup
0 3 * * * root /usr/local/bin/backup.sh | /usr/local/bin/transistor -b $(cat /var/run/backup-enabled) | /usr/local/bin/upload.sh
```

### 6.4 Expect / interactive control

You can change the base on each invocation; there is no persistent state inside the process.

```bash
for b in 0 1 0 1; do
    echo "base=$b"
    cat payload | transistor -b $b | wc -c
done
```

---

## 7. Safety & Interlock Patterns

These patterns treat the transistor as a deliberate safety gate.

### 7.1 Armed / disarmed flag

```bash
# Operator must create /tmp/armed before anything dangerous can leave the machine
dangerous_generator | transistor -b $(test -f /tmp/armed && echo 1 || echo 0) | network_send
```

### 7.2 Dead-man switch

```bash
# A watchdog process periodically touches /tmp/alive
# If the file disappears, the transistor cuts the stream
producer | transistor -b $(test -f /tmp/alive && echo 1 || echo 0) | consumer
```

### 7.3 Time-window gate

```bash
# Only allow traffic between 09:00 and 17:00
HOUR=$(date +%H)
BASE=0
if [ "$HOUR" -ge 9 ] && [ "$HOUR" -lt 17 ]; then BASE=1; fi
stream | transistor -b "$BASE" | destination
```

### 7.4 Permission / capability gate

```bash
# Only users in a certain group may pass data
BASE=$(id -Gn | grep -q special && echo 1 || echo 0)
data | transistor -b "$BASE" | privileged_sink
```

### 7.5 Fail-safe default

Always default the base to 0 (cutoff) unless an explicit positive signal is present:

```bash
BASE=${CONTROL:-0}          # if CONTROL is unset → 0
data | transistor -b "$BASE" | sink
```

---

## 8. Creative / Artistic Uses

### 8.1 Byte-level “volume” control for generative art

```bash
# Turn a noise source up or down
noise_generator | transistor -m amplify -b "$VOLUME" | visualizer
```

### 8.2 Binary “shutter” for a camera pipeline

```bash
# Capture frames only while the shutter flag is high
camera | transistor -b "$SHUTTER" | encoder
```

### 8.3 Data sculpture / selective destruction

```bash
# Randomly destroy or preserve chunks of a file
for i in $(seq 1 100); do
    dd if=bigfile bs=1M skip=$i count=1 2>/dev/null \
        | transistor -b $((RANDOM % 2)) \
        >> sculpture.bin
done
```

### 8.4 Musical gate (note on/off)

```bash
# Feed a stream of sample bytes; gate them with a note-on signal
synth | transistor -b "$NOTE_ON" | dac
```

---

## 9. Performance Notes

- The implementation uses an 8 KiB buffer and `BufReader` / `BufWriter`.
- In switch-on mode it is essentially a fast `cat`.
- In switch-off mode it still reads the entire stream (so the producer is not blocked by a full pipe) but discards the data.
- Amplify mode performs a simple multiply-and-clamp per byte; it is still extremely fast for most workloads.
- No heap allocations occur after startup.
- Suitable for high-throughput pipes (hundreds of MiB/s on typical hardware).

---

## 10. Edge Cases & Gotchas

| Situation | Behaviour |
|-----------|-----------|
| Empty stdin | Both modes produce empty stdout (correct) |
| Base = 0 in amplify mode | Every output byte becomes 0 |
| Base > 255 or non-numeric | Program exits with status 1 and an error on stderr |
| Broken pipe on stdout | Write errors are ignored (normal Unix filter behaviour) |
| Very large streams | Fully streamed; constant memory |
| Binary data containing nulls | Handled correctly (byte-oriented, not line-oriented) |
| Running under `sudo` or different users | No special privileges required or granted |

---

## 11. Comparison with Classic Unix Tools

| Goal | Classic tool | transistor equivalent |
|------|--------------|-----------------------|
| Copy stream | `cat` | `transistor -b 1` |
| Discard stream | `cat > /dev/null` | `transistor -b 0` |
| Conditional copy | `if ...; then cat; fi` | `transistor -b "$FLAG"` |
| Scale bytes | custom awk/perl | `transistor -m amplify -b N` |
| Safety interlock | complex shell | single transistor invocation |

The transistor does not replace `grep`, `sed`, `awk`, etc. It is deliberately a pure gain/switch element that can be dropped into any pipeline.

---

## 12. Extending the Metaphor

The current implementation is a **single** transistor.  
Interesting future directions (left as exercises):

- **Darlington pair** — two transistors in cascade with different thresholds.
- **Push-pull** — two complementary transistors driving a differential stream.
- **Multi-emitter** — one base controlling several independent output paths (requires more file descriptors or sockets).
- **Analog feedback** — read a portion of the output and adjust the base (would need a control channel).
- **Visual companion** — a small TUI that shows the conducting/cutoff state in real time as the base changes.

---

*End of usage guide.*
