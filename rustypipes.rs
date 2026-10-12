//! Scaled Plumbing System
//!
//! A concurrent byte-stream processor that reads from stdin, applies a
//! configurable XOR "cooling fin" mask, optionally duplicates the stream
//! (split mode), and writes the result to stdout.
//!
//! The design mirrors a physical pipe layout:
//!   Left vertical pipe (ingress) → Torus ring buffer → Right vertical pipe (egress)
//!
//! Usage examples:
//!   cargo run -- < input.bin > output.bin
//!   cargo run -- --capacity 512 --mask 0xA5 --mode split < input.bin
//!   echo "hello" | cargo run -- --mode split | xxd

use std::env;
use std::io::{self, Read, Write};
use std::process;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Defaults (chosen to mirror typical OpenSCAD geometry proportions)
// ---------------------------------------------------------------------------
const DEFAULT_OUTER_RADIUS: usize = 20;
const DEFAULT_TORUS_CAPACITY: usize = 220;
const DEFAULT_FIN_MASK: u8 = 0x5A;
const CHUNK_SIZE: usize = 8192;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------
#[derive(Debug, Clone)]
struct Config {
    torus_capacity: usize,
    fin_mask: u8,
    mode: Mode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Standard,
    Split,
}

impl Config {
    fn from_args() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut capacity = DEFAULT_TORUS_CAPACITY;
        let mut mask = DEFAULT_FIN_MASK;
        let mut mode = Mode::Standard;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-h" | "--help" => {
                    print_help(&args[0]);
                    process::exit(0);
                }
                "-c" | "--capacity" => {
                    if let Some(val) = args.get(i + 1) {
                        capacity = val.parse().unwrap_or_else(|_| {
                            eprintln!("error: invalid capacity value '{}'", val);
                            process::exit(1);
                        });
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
                            eprintln!("error: invalid mask value '{}' (expected hex, e.g. 0x5A)", val);
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
                            "split" => Mode::Split,
                            other => {
                                eprintln!("error: unknown mode '{}' (expected 'standard' or 'split')", other);
                                process::exit(1);
                            }
                        };
                        i += 2;
                    } else {
                        eprintln!("error: --mode requires a value");
                        process::exit(1);
                    }
                }
                unknown => {
                    eprintln!("error: unrecognized argument '{}'", unknown);
                    eprintln!("Try '{} --help' for more information.", args[0]);
                    process::exit(1);
                }
            }
        }

        Config {
            torus_capacity: capacity,
            fin_mask: mask,
            mode,
        }
    }
}

fn print_help(program: &str) {
    println!(
        "Scaled Plumbing System – concurrent stdin → stdout byte processor

USAGE:
    {program} [OPTIONS]

OPTIONS:
    -c, --capacity <BYTES>   Torus ring buffer capacity (default: {DEFAULT_TORUS_CAPACITY})
    -m, --mask <HEX>         Cooling-fin XOR mask (default: 0x{DEFAULT_FIN_MASK:02X})
        --mode <MODE>        Routing mode: 'standard' or 'split' (default: standard)
    -h, --help               Print this help message

MODES:
    standard   Apply XOR mask and forward the stream unchanged in length
    split      Apply XOR mask and duplicate every byte (dual-channel simulation)

EXAMPLES:
    {program} < data.bin > processed.bin
    {program} --capacity 512 --mask 0xA5 --mode split < data.bin
    echo -n \"hello\" | {program} --mode split | xxd
"
    );
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
fn main() {
    let config = Config::from_args();

    // Status messages always go to stderr so they never contaminate the pipe
    eprintln!("[Scaled Plumbing System]");
    eprintln!("  Outer radius proportion : {}", DEFAULT_OUTER_RADIUS);
    eprintln!("  Torus capacity          : {} bytes", config.torus_capacity);
    eprintln!("  Cooling fin mask        : 0x{:02X}", config.fin_mask);
    eprintln!("  Routing mode            : {:?}", config.mode);

    // Channels that simulate the physical pipe layout
    let (tx_ingress, rx_ingress) = mpsc::channel::<Vec<u8>>();
    let (tx_egress, rx_egress) = mpsc::channel::<Vec<u8>>();

    // ------------------------------------------------------------------
    // Stage 1 – Left vertical pipe (ingress from stdin)
    // ------------------------------------------------------------------
    let ingress = thread::spawn(move || {
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        let mut buf = vec![0u8; CHUNK_SIZE];

        loop {
            match handle.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    if tx_ingress.send(buf[..n].to_vec()).is_err() {
                        break; // Downstream closed
                    }
                }
                Err(e) => {
                    eprintln!("ingress error: {}", e);
                    break;
                }
            }
        }
    });

    // ------------------------------------------------------------------
    // Stage 2 – Toroidal ring + cooling fins (processing core)
    // ------------------------------------------------------------------
    let capacity = config.torus_capacity;
    let mask = config.fin_mask;
    let mode = config.mode;

    let processing = thread::spawn(move || {
        // We keep a fixed-size history only to honour the capacity constraint.
        // It is never read; it exists purely for the architectural metaphor.
        let mut ring = Vec::with_capacity(capacity);

        while let Ok(chunk) = rx_ingress.recv() {
            let mut out = Vec::with_capacity(match mode {
                Mode::Standard => chunk.len(),
                Mode::Split => chunk.len() * 2,
            });

            for &byte in &chunk {
                let cooled = byte ^ mask;

                // Enforce torus capacity (drop oldest if full)
                if ring.len() >= capacity {
                    ring.remove(0);
                }
                ring.push(cooled);

                out.push(cooled);
                if mode == Mode::Split {
                    out.push(cooled); // dual-stream duplication
                }
            }

            if tx_egress.send(out).is_err() {
                break;
            }
        }
    });

    // ------------------------------------------------------------------
    // Stage 3 – Right vertical pipe (egress to stdout)
    // ------------------------------------------------------------------
    let egress = thread::spawn(move || {
        let stdout = io::stdout();
        let mut handle = stdout.lock();

        while let Ok(chunk) = rx_egress.recv() {
            if let Err(e) = handle.write_all(&chunk) {
                eprintln!("egress error: {}", e);
                break;
            }
        }
        let _ = handle.flush();
    });

    // ------------------------------------------------------------------
    // Wait for clean shutdown
    // ------------------------------------------------------------------
    let _ = ingress.join();
    let _ = processing.join();
    let _ = egress.join();

    // Give a tiny moment so any final stderr messages are visible
    thread::sleep(Duration::from_millis(10));
    eprintln!("[Pipeline closed cleanly]");
}