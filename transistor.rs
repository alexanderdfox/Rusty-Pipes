//! transistor — a Linux-style pipe that behaves like a bipolar junction transistor
//!
//! Analogy:
//!   Collector  ← stdin  (data current)
//!   Base       ← control (command-line flag or first control byte)
//!   Emitter    → stdout (output current)
//!
//! Modes:
//!   - switch  (default): when base is high, pass data through (saturation);
//!                        when base is low, block / zero the stream (cutoff)
//!   - amplify: scale each byte by a gain factor derived from the base value
//!
//! The "ball with a hole" sits in the middle of the pipe. When the base
//! voltage is high the ball rotates so the hole aligns with the flow axis
//! and data passes. When low the solid side of the ball blocks the pipe.
//!
//! The provided button.stl is the outer body / flange shape of this
//! conceptual transistor-pipe fitting.

use std::env;
use std::io::{self, Read, Write, BufReader, BufWriter};
use std::process;

const DEFAULT_THRESHOLD: u8 = 1; // any non-zero base turns the transistor on

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Switch,
    Amplify,
}

fn print_help() {
    eprintln!(
r#"transistor — pipe that acts like a BJT transistor

USAGE:
    ... | transistor [OPTIONS] | ...

OPTIONS:
    -m, --mode <switch|amplify>   Operating mode (default: switch)
    -b, --base <0-255>            Base control value (default: 1)
                                  In switch mode: 0 = off, >0 = on
                                  In amplify mode: used as gain (0-255 maps to 0.0-2.0)
    -t, --threshold <0-255>       Threshold for "high" in switch mode (default: 1)
    -h, --help                    Show this help

EXAMPLES:
    # Pass data only when base is high (classic switch)
    cat data.bin | transistor --base 1 > out.bin

    # Block the stream
    cat data.bin | transistor --base 0 > /dev/null

    # Amplify (brighten) a stream of bytes by ~1.5×
    cat image.raw | transistor -m amplify -b 192 > brighter.raw

The physical model of this pipe uses the supplied button.stl as the
outer body. In the middle sits a ball with a hole drilled through it;
the base signal rotates the ball so the hole either aligns with the
pipe axis (conducting) or is perpendicular (blocking).
"#
    );
}

fn parse_args() -> (Mode, u8, u8) {
    let args: Vec<String> = env::args().collect();
    let mut mode = Mode::Switch;
    let mut base: u8 = DEFAULT_THRESHOLD;
    let mut threshold: u8 = DEFAULT_THRESHOLD;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                process::exit(0);
            }
            "-m" | "--mode" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --mode requires an argument");
                    process::exit(1);
                }
                match args[i].to_lowercase().as_str() {
                    "switch" => mode = Mode::Switch,
                    "amplify" => mode = Mode::Amplify,
                    other => {
                        eprintln!("error: unknown mode '{}'", other);
                        process::exit(1);
                    }
                }
            }
            "-b" | "--base" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --base requires a value 0-255");
                    process::exit(1);
                }
                base = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("error: invalid base value");
                    process::exit(1);
                });
            }
            "-t" | "--threshold" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --threshold requires a value 0-255");
                    process::exit(1);
                }
                threshold = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("error: invalid threshold");
                    process::exit(1);
                });
            }
            other => {
                eprintln!("error: unknown argument '{}'", other);
                print_help();
                process::exit(1);
            }
        }
        i += 1;
    }

    (mode, base, threshold)
}

fn main() {
    let (mode, base, threshold) = parse_args();

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());

    let mut buf = [0u8; 8192];

    match mode {
        Mode::Switch => {
            // Classic digital transistor: on or off
            let conducting = base >= threshold;
            if conducting {
                // Ball hole aligned → pass everything
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if writer.write_all(&buf[..n]).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            } else {
                // Ball solid side blocking → sink the stream silently
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(_) => {} // discard
                        Err(_) => break,
                    }
                }
            }
        }
        Mode::Amplify => {
            // Analog-ish: gain derived from base (0 → 0.0, 128 → 1.0, 255 → ~2.0)
            let gain = base as f32 / 128.0;
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        for b in &mut buf[..n] {
                            let v = (*b as f32 * gain).round().clamp(0.0, 255.0) as u8;
                            *b = v;
                        }
                        if writer.write_all(&buf[..n]).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }

    let _ = writer.flush();
}
