use std::env;
use std::f64::consts::PI;
use std::io::{self, BufRead, BufWriter, Write};

#[derive(Clone, Copy, Debug)]
struct Sample {
    t: f64,        // time in seconds
    dt: f64,       // time step in seconds
    pressure: f64, // analogous to voltage (V)
    flow: f64,     // analogous to current / flow rate (A / m^3/s)
}

impl Sample {
    fn parse(line: &str) -> Option<Self> {
        let mut parts = line.split_whitespace();
        let t: f64 = parts.next()?.parse().ok()?;
        let dt: f64 = parts.next()?.parse().ok()?;
        let pressure: f64 = parts.next()?.parse().ok()?;
        let flow: f64 = parts.next()?.parse().ok()?;
        Some(Self {
            t,
            dt,
            pressure,
            flow,
        })
    }

    fn format_line(&self) -> String {
        format!(
            "{:.9} {:.9} {:.6} {:.6}",
            self.t, self.dt, self.pressure, self.flow
        )
    }
}

/// Src1: 22 kHz oscillating pump (default 5V peak, 100 kHz sample rate)
fn run_src1(freq_hz: f64, amplitude: f64, sample_rate: f64, duration_s: f64) {
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let dt = 1.0 / sample_rate;
    let steps = (duration_s * sample_rate).round() as usize;

    for i in 0..steps {
        let t = i as f64 * dt;
        // Pressure oscillates relative to 0 V ground; initial flow estimation
        let pressure = amplitude * (2.0 * PI * freq_hz * t).sin();
        let flow = amplitude * (2.0 * PI * freq_hz * t).cos() * 0.01;
        let s = Sample {
            t,
            dt,
            pressure,
            flow,
        };
        let _ = writeln!(out, "{}", s.format_line());
    }
}

/// L1 / L2: Hydraulic momentum coil (inductor: L * dQ/dt = dP)
/// Resists rapid changes in flow rate; acts like fluid inertia in a pipe.
fn run_inductor(inductance_h: f64, shunt: bool) {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let mut flow_state = 0.0_f64;
    let mut prev_pressure = 0.0_f64;
    let mut first = true;

    for line in stdin.lock().lines().map_while(Result::ok) {
        let Some(mut s) = Sample::parse(&line) else {
            continue;
        };
        if first {
            prev_pressure = s.pressure;
            first = false;
        }

        let dp = s.pressure - prev_pressure;
        // dQ = (dP / L) * dt (damped slightly for numerical stability)
        let dflow = ((dp / inductance_h.max(1e-12)) * s.dt) - (flow_state * 0.05);
        flow_state += dflow;
        prev_pressure = s.pressure;

        if shunt {
            // L2 shunts flow toward the return line, slightly loading main pressure
            s.pressure *= 1.0 / (1.0 + (s.dt / inductance_h.max(1e-12)) * 1e-4);
            s.flow = (s.flow - flow_state * 0.1).clamp(-100.0, 100.0);
        } else {
            // L1 is in series: smooths flow through the pipe
            s.flow = flow_state.clamp(-100.0, 100.0);
        }

        let _ = writeln!(out, "{}", s.format_line());
    }
}

/// C1 / C2: Elastic pressure bladder (capacitor: Q_bladder = C * P, i = C * dP/dt)
/// Absorbs flow when pressure rises; releases flow when pressure drops.
fn run_capacitor(capacitance_f: f64) {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let mut prev_pressure = 0.0_f64;
    let mut first = true;

    for line in stdin.lock().lines().map_while(Result::ok) {
        let Some(mut s) = Sample::parse(&line) else {
            continue;
        };
        if first {
            prev_pressure = s.pressure;
            first = false;
        }

        let dp_dt = if s.dt > 0.0 {
            (s.pressure - prev_pressure) / s.dt
        } else {
            0.0
        };
        let bladder_flow = capacitance_f * dp_dt;
        prev_pressure = s.pressure;

        // Downstream flow is what remains after the bladder takes/gives fluid
        s.flow -= bladder_flow;
        let _ = writeln!(out, "{}", s.format_line());
    }
}

/// SW1 / SW2: Inline gate valve (switch)
fn run_switch(open: bool) {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    for line in stdin.lock().lines().map_while(Result::ok) {
        let Some(mut s) = Sample::parse(&line) else {
            continue;
        };
        if !open {
            // Closed valve blocks flow and isolates downstream pressure (0 V)
            s.flow = 0.0;
            s.pressure = 0.0;
        }
        let _ = writeln!(out, "{}", s.format_line());
    }
}

/// Vout: Pressure gauge relative to 0 V ground
fn run_vout() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let _ = writeln!(out, "time_s\tVout_V\tflow_A");
    for line in stdin.lock().lines().map_while(Result::ok) {
        let Some(s) = Sample::parse(&line) else {
            continue;
        };
        // Measured relative to 0 V ground reference
        let vout = s.pressure - 0.0;
        let _ = writeln!(out, "{:.7}\t{:+.4}\t{:+.4}", s.t, vout, s.flow);
    }
}

fn print_usage() {
    eprintln!(
        "Usage: circuit_pipe <stage> [options]
Stages (plumbing stream over stdin/stdout):
  src1 [--freq 22000] [--amp 5.0] [--sr 200000] [--dur 0.0002]
  l1   [--l 0.00047]      (470 uH series momentum coil)
  c1   [--c 0.000001]     (1 uF elastic pressure bladder)
  sw1  [--closed]         (valve 1; open by default)
  c2   [--c 0.000001]     (1 uF elastic pressure bladder)
  sw2  [--closed]         (valve 2; open by default)
  l2   [--l 0.00047]      (470 uH shunt momentum choke)
  vout                    (output pressure gauge vs 0V)"
    );
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        return;
    }

    match args[0].as_str() {
        "src1" => {
            let mut freq = 22_000.0;
            let mut amp = 5.0;
            let mut sr = 200_000.0;
            let mut dur = 0.0002; // ~4.4 cycles at 22 kHz
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--freq" if i + 1 < args.len() => freq = args[i + 1].parse().unwrap_or(freq),
                    "--amp" if i + 1 < args.len() => amp = args[i + 1].parse().unwrap_or(amp),
                    "--sr" if i + 1 < args.len() => sr = args[i + 1].parse().unwrap_or(sr),
                    "--dur" if i + 1 < args.len() => dur = args[i + 1].parse().unwrap_or(dur),
                    _ => {}
                }
                i += 2;
            }
            run_src1(freq, amp, sr, dur);
        }
        "l1" => {
            let l = args
                .windows(2)
                .find(|w| w[0] == "--l")
                .and_then(|w| w[1].parse().ok())
                .unwrap_or(470e-6);
            run_inductor(l, false);
        }
        "c1" | "c2" => {
            let c = args
                .windows(2)
                .find(|w| w[0] == "--c")
                .and_then(|w| w[1].parse().ok())
                .unwrap_or(1e-6);
            run_capacitor(c);
        }
        "sw1" | "sw2" => {
            let closed = args.iter().any(|a| a == "--closed");
            run_switch(!closed);
        }
        "l2" => {
            let l = args
                .windows(2)
                .find(|w| w[0] == "--l")
                .and_then(|w| w[1].parse().ok())
                .unwrap_or(470e-6);
            run_inductor(l, true);
        }
        "vout" => run_vout(),
        _ => print_usage(),
    }
}