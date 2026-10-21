use std::env;
use std::io::{self, BufRead, Write};

const R: f64 = 10_000.0;
const GAIN: f64 = 100_000.0;
const RAIL: f64 = 15.0;
const VF: f64 = 0.0;

fn process_mode1(vin: f64) -> f64 {
    let v_plus = 0.0;
    let i = vin / R;
    if i >= 0.0 {
        -VF
    } else {
        let v_minus = vin;
        let v_diff = v_plus - v_minus;
        (-RAIL).max(RAIL.min(GAIN * v_diff))
    }
}

fn process_mode2(vin: f64) -> f64 {
    if vin > 0.0 {
        -vin
    } else {
        0.0
    }
}

fn print_usage() {
    eprintln!("Usage: diode [--mode 1|2]");
    eprintln!("  --mode 1 : Feedback diode circuit model (default)");
    eprintln!("  --mode 2 : Input diode circuit model");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut mode = 1;

    let mut i = 1;
    while i < args.len() {
        if args[i] == "--mode" || args[i] == "-m" {
            if i + 1 < args.len() {
                if let Ok(m) = args[i + 1].parse::<i32>() {
                    mode = m;
                }
                i += 2;
                continue;
            }
        } else if args[i] == "--help" || args[i] == "-h" {
            print_usage();
            std::process::exit(0);
        }
        i += 1;
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    for line_result in stdin.lock().lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        match trimmed.parse::<f64>() {
            Ok(vin) => {
                let result = match mode {
                    2 => process_mode2(vin),
                    _ => process_mode1(vin),
                };
                if let Err(_) = writeln!(stdout_lock, "{}", result) {
                    break;
                }
            }
            Err(_) => {
                eprintln!("# skip bad sample: {}", trimmed);
            }
        }
    }
}
