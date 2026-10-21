# Rusty-Pipes

**Metaphor-driven concurrent byte-stream processors in Rust.**

These tools treat data pipelines as physical plumbing and electronic circuits.  
Each component is a (nearly) pure filter that can be dropped into classic Unix pipelines.

```
Left vertical pipe → Torus ring + cooling fins → Right vertical pipe
Collector ← Base → Emitter          (BJT transistor)
West ──► XOR ◄── North → East       (cross-pipe)
```

---

## Quick Start

```bash
# Build everything
cargo build --release

# The four binaries will appear in target/release/
ls target/release/{xpipe,rustypipes,transistor,diode}
```

---

## Components

### 1. `xpipe` — Topological Cross-Pipe

Multi-stream router and XOR encryptor.

```
          North (keystream / second stream)
             │
West ───────►┼───────► East   (XOR result)
             │
          South (leftover / unmatched data)
```

**Features**
- Bounded channels for back-pressure
- Handles uneven chunk sizes cleanly
- Unmatched data is flushed to South (stderr by default)

```bash
# Basic XOR encryption
./target/release/xpipe \
  --west plaintext.bin \
  --north keystream.bin \
  --east ciphertext.bin

# Using the helper script
./scripts/xencrypt.sh encrypt input.bin output.bin key.bin
./scripts/xencrypt.sh decrypt output.bin recovered.bin key.bin
```

CLI flags:
| Flag | Description | Default |
|------|-------------|---------|
| `--west` | Input A (file or stdin) | stdin |
| `--north` | Input B (file) | required for XOR |
| `--east` | Primary output | stdout |
| `--south` | Leftover data | stderr |
| `--buffer-size` | Channel capacity | 1000 |

---

### 2. `rustypipes` — Scaled Plumbing System

Concurrent three-stage pipeline that mirrors a physical pipe layout:

```
Left vertical pipe (ingress)
        ↓
Torus ring buffer + cooling fins (XOR mask)
        ↓
Right vertical pipe (egress)
```

**Modes**
- `standard` — apply XOR mask, 1:1 length
- `split` — apply XOR mask and duplicate every byte (dual-channel simulation)

```bash
# Simple processing
echo -n "hello" | ./target/release/rustypipes --mode split | xxd

# Custom capacity and mask
./target/release/rustypipes \
  --capacity 512 \
  --mask 0xA5 \
  --mode standard < data.bin > processed.bin
```

| Flag | Description | Default |
|------|-------------|---------|
| `-c, --capacity` | Torus ring-buffer size | 220 |
| `-m, --mask` | XOR “cooling-fin” mask (hex) | `0x5A` |
| `--mode` | `standard` or `split` | `standard` |

Defaults were chosen to mirror typical OpenSCAD geometry proportions used in the related 3-D pipe models.

---

### 3. `transistor` — BJT-style Pipe Filter

A Linux-style filter that behaves like a bipolar junction transistor.

| Terminal  | Role                     |
|-----------|--------------------------|
| Collector | stdin  (data current)    |
| Base      | CLI control value        |
| Emitter   | stdout (output current)  |

**Modes**
- **switch** (default) — classic on/off gate  
  `base ≥ threshold` → pass data  
  `base <  threshold` → discard data
- **amplify** — scale every byte by `base / 128.0` (clamped 0–255)

```bash
# Digital switch
cat data.bin | ./target/release/transistor --base 1 > out.bin   # pass
cat data.bin | ./target/release/transistor --base 0 > /dev/null # block

# Amplify / brighten
cat image.raw | ./target/release/transistor -m amplify -b 192 > brighter.raw
```

Full elaborate usage guide (pipeline patterns, safety interlocks, artistic uses, etc.) is in  
[`docs/transistor.md`](docs/transistor.md).

---

### 4. `diode` — Circuit Model Filter

Simple numeric stream processors that model diode / op-amp circuits.

```bash
# Feedback diode model (default)
seq -f "%.1f" -5 0.5 5 | ./target/release/diode --mode 1

# Input diode model
seq -f "%.1f" -5 0.5 5 | ./target/release/diode --mode 2
```

| Mode | Description |
|------|-------------|
| 1 | Feedback diode circuit model (default) |
| 2 | Input diode circuit model |

---

## Project Layout

```
Rusty-Pipes/
├── Cargo.toml              # Multi-binary package
├── LICENSE                 # MIT
├── README.md               # This file
├── src/
│   └── main.rs             # xpipe
├── bins/
│   ├── rustypipes.rs       # Scaled Plumbing System
│   ├── transistor.rs       # BJT filter
│   └── diode.rs            # Diode / op-amp models
├── docs/
│   └── transistor.md       # Full transistor usage guide
└── scripts/
    └── xencrypt.sh         # Convenience wrapper for xpipe
```

All four tools are proper Cargo binaries. Build once with `cargo build --release` and they appear together in `target/release/`.

---

## Design Philosophy

- **Pure (or nearly pure) filters** — diagnostics always go to stderr so they never pollute data pipes.
- **Explicit physical / electronic metaphors** — pipes, toroids, cooling fins, BJTs, diodes.
- **Concurrent where it adds value** — ring buffers, multi-stream matching, bounded channels.
- **Constant or bounded memory** — suitable for high-throughput streaming.
- **Composable** — designed to be dropped into classic Unix pipelines and shell scripts.

---

## License

MIT © 2026 Alex Fox
