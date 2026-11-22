use std::env;
use std::f64::consts::PI;
use std::io::{self, BufRead, Write};
use std::process;

#[derive(Clone, Copy, Debug)]
struct Sample {
    t: f64,
    signal: f64,
    current: f64,
}

impl Sample {
    fn from_line(line: &str) -> Option<Self> {
        let mut parts = line.split_whitespace();
        Some(Sample {
            t: parts.next()?.parse().ok()?,
            signal: parts.next()?.parse().ok()?,
            current: parts.next()?.parse().ok()?,
        })
    }

    fn write(&self, out: &mut impl Write) -> io::Result<()> {
        writeln!(out, "{:.9} {:+.6} {:+.6}", self.t, self.signal, self.current)
    }
}

// ── Stages ──────────────────────────────────────────────────────────────────

fn ground(sample_rate: f64, num_samples: usize) {
    let mut out = io::stdout().lock();
    for i in 0..num_samples {
        let s = Sample {
            t: i as f64 / sample_rate,
            signal: 0.0,
            current: 0.0,
        };
        if s.write(&mut out).is_err() {
            break;
        }
    }
}

fn ac_source(freq_hz: f64, amplitude: f64) {
    let stdin = io::stdin();
    let mut out = io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let mut s = match Sample::from_line(&line) {
            Some(s) => s,
            None => continue,
        };

        let wave = amplitude * (2.0 * PI * freq_hz * s.t).sin();
        s.signal += wave;
        s.current += wave * 0.01;

        if s.write(&mut out).is_err() {
            break;
        }
    }
}

fn capacitor(capacitance_f: f64, dt: f64) {
    let stdin = io::stdin();
    let mut out = io::stdout().lock();

    let mut prev_signal = 0.0;
    let mut dc_estimate = 0.0;
    let alpha = 0.05;

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let mut s = match Sample::from_line(&line) {
            Some(s) => s,
            None => continue,
        };

        dc_estimate = (1.0 - alpha) * dc_estimate + alpha * s.signal;
        let dv = s.signal - prev_signal;
        let cap_current = capacitance_f * (dv / dt);

        prev_signal = s.signal;
        s.signal -= dc_estimate; // AC-coupled
        s.current = cap_current;

        if s.write(&mut out).is_err() {
            break;
        }
    }
}

fn inductor(inductance_h: f64, dt: f64) {
    let stdin = io::stdin();
    let mut out = io::stdout().lock();

    let mut flux_current = 0.0;

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let mut s = match Sample::from_line(&line) {
            Some(s) => s,
            None => continue,
        };

        // Integrate voltage into current response
        flux_current += (s.signal / inductance_h) * dt * 1e6;
        s.current = flux_current;

        if s.write(&mut out).is_err() {
            break;
        }
    }
}

// ── CLI ─────────────────────────────────────────────────────────────────────

fn usage() -> ! {
    eprintln!(
        "Usage:
  circuit ground <sample_rate> <num_samples>
  circuit ac     <freq_hz> <amplitude>
  circuit cap    <capacitance_F> <dt>
  circuit ind    <inductance_H> <dt>

Examples:
  circuit ground 200000 20
  circuit ac 22000 5
  circuit cap 470e-6 5e-6
  circuit ind 1.0 5e-6"
    );
    process::exit(1);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        usage();
    }

    match args[1].as_str() {
        "ground" => {
            if args.len() != 4 {
                usage();
            }
            let sample_rate: f64 = args[2].parse().unwrap_or_else(|_| usage());
            let num_samples: usize = args[3].parse().unwrap_or_else(|_| usage());
            ground(sample_rate, num_samples);
        }
        "ac" => {
            if args.len() != 4 {
                usage();
            }
            let freq: f64 = args[2].parse().unwrap_or_else(|_| usage());
            let amp: f64 = args[3].parse().unwrap_or_else(|_| usage());
            ac_source(freq, amp);
        }
        "cap" => {
            if args.len() != 4 {
                usage();
            }
            let c: f64 = args[2].parse().unwrap_or_else(|_| usage());
            let dt: f64 = args[3].parse().unwrap_or_else(|_| usage());
            capacitor(c, dt);
        }
        "ind" => {
            if args.len() != 4 {
                usage();
            }
            let l: f64 = args[2].parse().unwrap_or_else(|_| usage());
            let dt: f64 = args[3].parse().unwrap_or_else(|_| usage());
            inductor(l, dt);
        }
        _ => usage(),
    }
}
