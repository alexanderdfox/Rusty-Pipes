# Circuit – Analog Stages as Linux Pipes

A small Rust tool that turns classic analog circuit elements (ground, AC sources, capacitors, inductors) into pure Unix filters.  
Each stage reads a stream of samples from **stdin** and writes the transformed stream to **stdout**, so you can compose complete signal paths with the real shell pipe operator `|`.

This is the same philosophy as the classic Unix tools: small, composable, text-stream oriented.

---

## Protocol

Every stage speaks the same simple line-based protocol:

```
t signal current
```

- `t`      – time in seconds  
- `signal` – voltage / normalized amplitude  
- `current`– modeled current  

Example line:
```
0.000050 +4.755282 -0.012345
```

Empty lines and malformed lines are ignored.

---

## Building

```bash
rustc -O circuit.rs -o circuit
```

Or with Cargo:

```bash
cargo init --name circuit
# put the source in src/main.rs
cargo build --release
# binary is at target/release/circuit
```

---

## Stages

| Command                          | Description                              | Parameters                          |
|----------------------------------|------------------------------------------|-------------------------------------|
| `circuit ground <rate> <n>`      | Generates a pure ground (0 V) reference  | sample rate (Hz), number of samples |
| `circuit ac <freq> <amp>`        | Adds a sine-wave AC source               | frequency (Hz), amplitude (V)       |
| `circuit cap <C> <dt>`           | Capacitor (AC-coupling / high-pass)      | capacitance (F), time step (s)      |
| `circuit ind <L> <dt>`           | Inductor (opposes change in current)     | inductance (H), time step (s)       |

`dt` is normally `1 / sample_rate`.

---

## Basic Usage

Generate a pure ground reference:

```bash
./circuit ground 200000 20
```

Add a 22 kHz, 5 V sine wave:

```bash
./circuit ground 200000 20 | ./circuit ac 22000 5
```

Full original topology from the original program:

```
GND → 22 kHz AC (5 V) → 470 µF → 1 H → 22 kHz AC (3 V) → 1 H → 470 µF → 22 kHz AC (1 V)
```

```bash
./circuit ground 200000 30 \
  | ./circuit ac 22000 5 \
  | ./circuit cap 470e-6 5e-6 \
  | ./circuit ind 1.0 5e-6 \
  | ./circuit ac 22000 3 \
  | ./circuit ind 1.0 5e-6 \
  | ./circuit cap 470e-6 5e-6 \
  | ./circuit ac 22000 1
```

Pretty-print the output:

```bash
./circuit ground 200000 30 \
  | ./circuit ac 22000 5 \
  | ./circuit cap 470e-6 5e-6 \
  | ./circuit ind 1.0 5e-6 \
  | ./circuit ac 22000 3 \
  | ./circuit ind 1.0 5e-6 \
  | ./circuit cap 470e-6 5e-6 \
  | ./circuit ac 22000 1 \
  | awk '{printf "#%02d  t=%.6fs  sig=%+8.3f  i=%+10.4f\n", NR-1, $1, $2, $3}'
```

---

## Useful Patterns

**Inspect intermediate stages**

```bash
./circuit ground 200000 40 \
  | ./circuit ac 1000 2 \
  | tee /tmp/before-cap.txt \
  | ./circuit cap 470e-6 5e-6 \
  | tee /tmp/after-cap.txt
```

**Save a long run**

```bash
./circuit ground 200000 1000 \
  | ./circuit ac 22000 5 \
  | ./circuit cap 470e-6 5e-6 \
  | ./circuit ind 1.0 5e-6 \
  > samples.txt
```

**Quick frequency experiment**

```bash
# 1 kHz vs 22 kHz through the same capacitor
./circuit ground 200000 50 | ./circuit ac 1000 3  | ./circuit cap 470e-6 5e-6
./circuit ground 200000 50 | ./circuit ac 22000 3 | ./circuit cap 470e-6 5e-6
```

**Pipe into other tools**

```bash
# feed into gnuplot, awk, python, etc.
./circuit ground 200000 200 | ./circuit ac 5000 2 | ./circuit cap 100e-6 5e-6 \
  | awk '{print $1, $2}' | gnuplot -e "set terminal dumb; plot '-' with lines"
```

---

## Design Notes

- Every stage is a pure filter: it never knows what is upstream or downstream.
- State (previous voltage, accumulated flux, DC estimate) lives only inside the process.
- Because the protocol is plain text, you can insert `tee`, `head`, `grep`, `awk`, `sed`, or even other languages anywhere in the chain.
- The same binary is used for every stage; the first argument selects the behaviour (classic Unix style).

This turns an analog signal path into something you can explore, debug and re-wire with ordinary shell tools.
```
