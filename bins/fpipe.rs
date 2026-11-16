//! fpipe — Lowercase-f Pipe Filter
//!
//! Models the parametric OpenSCAD “f” pipe as a concurrent byte-stream processor.
//! Topology:
//!
//!           [hook tip → stdout]
//!                ╭──
//!                │
//!      [upper]   │
//!                │
//!   left ───────●─────── right     ← crossbar (open ends)
//!                │
//!              [bottom]
//!                │
//!             [stem in ← stdin]
//!
//! Circuit analogy: series R per segment, shunt C ∝ volume, junction node.
//! The ring buffer realises the total capacitance; the mask is the wall texture.
//!
//! Usage:
//!   cargo run --bin fpipe -- < data.bin > out.bin
//!   echo -n "hello" | cargo run --bin fpipe -- --mode branch | xxd
//!   cargo run --bin fpipe -- --capacity 512 --mask 0xA5 --mode standard

use std::env;
use std::io::{self, Read, Write};
use std::process;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Defaults taken directly from the OpenSCAD parametric “f”
// ---------------------------------------------------------------------------
const DEFAULT_STEM_HEIGHT: f64 = 60.0;
const DEFAULT_CROSS_Y: f64 = 38.0;
const DEFAULT_CROSS_LEN: f64 = 26.0;
const DEFAULT_HOOK_RADIUS: f64 = 15.0;
const DEFAULT_HOOK_ANGLE: f64 = 110.0;
const DEFAULT_INNER_R: f64 = 4.5; // outer 6 − wall 1.5
const DEFAULT_MASK: u8 = 0x5A;
const CHUNK_SIZE: usize = 8192;

// Scale factor so volumes become a reasonable byte capacity
const VOL_TO_BYTES: f64 = 0.8;

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------
fn hook_arc_len(radius: f64, angle_deg: f64) -> f64 {
    (angle_deg / 360.0) * 2.0 * std::f64::consts::PI * radius
}

fn segment_volume(len: f64, r: f64) -> f64 {
    len * std::f64::consts::PI * r * r
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------
#[derive(Debug, Clone)]
struct Config {
    capacity: usize,
    mask: u8,
    mode: Mode,
    // retained for status / future R-weighted transfer
    lower_len: f64,
    upper_len: f64,
    cross_len: f64,
    hook_len: f64,
    total_vol: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Standard, // primary path only (stem → upper → hook)
    Branch,   // emulate open crossbar ends by duplicating the stream
}

impl Config {
    fn from_args() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut capacity_override: Option<usize> = None;
        let mut mask = DEFAULT_MASK;
        let mut mode = Mode::Standard;

        // Geometry overrides (still default to the SCAD values)
        let mut stem_h = DEFAULT_STEM_HEIGHT;
        let mut cross_y = DEFAULT_CROSS_Y;
        let mut cross_l = DEFAULT_CROSS_LEN;
        let mut hook_r = DEFAULT_HOOK_RADIUS;
        let mut hook_a = DEFAULT_HOOK_ANGLE;
        let mut inner_r = DEFAULT_INNER_R;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-h" | "--help" => {
                    print_help(&args[0]);
                    process::exit(0);
                }
                "-c" | "--capacity" => {
                    if let Some(val) = args.get(i + 1) {
                        capacity_override = Some(val.parse().unwrap_or_else(|_| {
                            eprintln!("error: invalid capacity '{}'", val);
                            process::exit(1);
                        }));
                        i += 2;
                    } else {
                        eprintln!("error: --capacity requires a value");
                        process::exit(1);
                    }
                }
                "-m" | "--mask" => {
                    if let Some(val) = args.get(i + 1) {
                        let cleaned = val.trim_start_matches("0x").trim_start_matches("0X");
                        mask = u8::from_str_radix(cleaned, 16).unwrap_or_else(|_| {
                            eprintln!("error: invalid mask '{}' (hex expected)", val);
                            process::exit(1);
                        });
                        i += 2;
                    } else {
                        eprintln!("error: --mask requires a value");
                        process::exit(1);
                    }
                }
                "--mode" => {
                    if let Some(val) = args.get(i + 1) {
                        mode = match val.as_str() {
                            "standard" => Mode::Standard,
                            "branch" => Mode::Branch,
                            other => {
                                eprintln!("error: unknown mode '{}' (standard|branch)", other);
                                process::exit(1);
                            }
                        };
                        i += 2;
                    } else {
                        eprintln!("error: --mode requires a value");
                        process::exit(1);
                    }
                }
                // Optional geometry overrides (same names as the SCAD)
                "--stem-height" => {
                    if let Some(val) = args.get(i + 1) {
                        stem_h = val.parse().unwrap_or(stem_h);
                        i += 2;
                    } else { i += 1; }
                }
                "--cross-y" => {
                    if let Some(val) = args.get(i + 1) {
                        cross_y = val.parse().unwrap_or(cross_y);
                        i += 2;
                    } else { i += 1; }
                }
                "--cross-len" => {
                    if let Some(val) = args.get(i + 1) {
                        cross_l = val.parse().unwrap_or(cross_l);
                        i += 2;
                    } else { i += 1; }
                }
                "--hook-radius" => {
                    if let Some(val) = args.get(i + 1) {
                        hook_r = val.parse().unwrap_or(hook_r);
                        i += 2;
                    } else { i += 1; }
                }
                "--hook-angle" => {
                    if let Some(val) = args.get(i + 1) {
                        hook_a = val.parse().unwrap_or(hook_a);
                        i += 2;
                    } else { i += 1; }
                }
                unknown => {
                    eprintln!("error: unrecognized argument '{}'", unknown);
                    eprintln!("Try '{} --help' for more information.", args[0]);
                    process::exit(1);
                }
            }
        }

        let lower = cross_y;
        let upper = (stem_h - cross_y).max(0.0);
        let hook = hook_arc_len(hook_r, hook_a);
        let total_vol = segment_volume(lower, inner_r)
            + segment_volume(upper, inner_r)
            + segment_volume(cross_l, inner_r)
            + segment_volume(hook, inner_r);

        let capacity = capacity_override.unwrap_or_else(|| {
            ((total_vol * VOL_TO_BYTES).round() as usize).max(64)
        });

        Config {
            capacity,
            mask,
            mode,
            lower_len: lower,
            upper_len: upper,
            cross_len: cross_l,
            hook_len: hook,
            total_vol,
        }
    }
}

fn print_help(program: &str) {
    println!(
        "fpipe – Lowercase-f Pipe Filter (OpenSCAD geometry → Unix filter)

USAGE:
    {program} [OPTIONS]

OPTIONS:
    -c, --capacity <BYTES>   Ring-buffer capacity (default: derived from geometry)
    -m, --mask <HEX>         Wall-texture XOR mask (default: 0x{DEFAULT_MASK:02X})
        --mode <MODE>        standard | branch (default: standard)
        --stem-height <mm>   (default: {DEFAULT_STEM_HEIGHT})
        --cross-y <mm>       (default: {DEFAULT_CROSS_Y})
        --cross-len <mm>     (default: {DEFAULT_CROSS_LEN})
        --hook-radius <mm>   (default: {DEFAULT_HOOK_RADIUS})
        --hook-angle <deg>   (default: {DEFAULT_HOOK_ANGLE})
    -h, --help               Print this help

MODES:
    standard   Primary path only (stem → upper → hook → stdout)
    branch     Emulate the two open crossbar ends by duplicating every byte

CIRCUIT ANALOGY:
    Segments = series R, volumes = shunt C, crossbar junction = KCL node.
    The ring buffer realises total C; the mask is surface effect.

EXAMPLES:
    {program} < data.bin > out.bin
    {program} --mode branch --mask 0xA5 < data.bin | xxd
    echo -n \"flow\" | {program} --capacity 128
"
    );
}

// ---------------------------------------------------------------------------
// Main – three concurrent stages mirroring the geometry
// ---------------------------------------------------------------------------
fn main() {
    let config = Config::from_args();

    eprintln!("[fpipe – lowercase-f manifold]");
    eprintln!("  lower stem     : {:.1} mm", config.lower_len);
    eprintln!("  upper stem     : {:.1} mm", config.upper_len);
    eprintln!("  crossbar       : {:.1} mm", config.cross_len);
    eprintln!("  hook arc       : {:.2} mm", config.hook_len);
    eprintln!("  total volume   : {:.1} mm³", config.total_vol);
    eprintln!("  ring capacity  : {} bytes  (C)", config.capacity);
    eprintln!("  wall mask      : 0x{:02X}", config.mask);
    eprintln!("  routing mode   : {:?}", config.mode);

    let (tx_in, rx_in) = mpsc::channel::<Vec<u8>>();
    let (tx_junc, rx_junc) = mpsc::channel::<Vec<u8>>();
    let (tx_out, rx_out) = mpsc::channel::<Vec<u8>>();

    // ------------------------------------------------------------------
    // Stage 1 – Bottom stem (ingress)
    // ------------------------------------------------------------------
    let ingress = thread::spawn(move || {
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        let mut buf = vec![0u8; CHUNK_SIZE];
        loop {
            match handle.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx_in.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("bottom-stem error: {}", e);
                    break;
                }
            }
        }
    });

    // ------------------------------------------------------------------
    // Stage 2 – Junction + crossbar + upper stem (processing core)
    // The ring realises the capacitive volume of the whole manifold.
    // ------------------------------------------------------------------
    let capacity = config.capacity;
    let mask = config.mask;
    let mode = config.mode;

    let junction = thread::spawn(move || {
        let mut ring: Vec<u8> = Vec::with_capacity(capacity);

        while let Ok(chunk) = rx_in.recv() {
            let mut out = Vec::with_capacity(match mode {
                Mode::Standard => chunk.len(),
                Mode::Branch => chunk.len() * 2,
            });

            for &b in &chunk {
                let cooled = b ^ mask;

                // Capacitor behaviour: bounded volume
                if ring.len() >= capacity {
                    ring.remove(0);
                }
                ring.push(cooled);

                out.push(cooled);
                if mode == Mode::Branch {
                    // open left + right crossbar ends
                    out.push(cooled);
                }
            }

            if tx_junc.send(out).is_err() {
                break;
            }
        }
    });

    // ------------------------------------------------------------------
    // Stage 3 – Hook (egress)
    // ------------------------------------------------------------------
    let egress = thread::spawn(move || {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        while let Ok(chunk) = rx_junc.recv() {
            if let Err(e) = handle.write_all(&chunk) {
                eprintln!("hook-tip error: {}", e);
                break;
            }
        }
        let _ = handle.flush();
    });

    let _ = ingress.join();
    let _ = junction.join();
    let _ = egress.join();

    thread::sleep(Duration::from_millis(10));
    eprintln!("[fpipe closed cleanly]");
}