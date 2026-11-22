// tool.rs  –  build with:  rustc -O tool.rs -o tool
// then:  ln -s tool toolpipe ; ln -s tool tooltree
// or just call  ./tool pipe ...   and  ./tool tree ...

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
	fn is_finite(&self) -> bool {
		self.t.is_finite() && self.signal.is_finite() && self.current.is_finite()
	}

	fn from_line(line: &str) -> Option<Self> {
		let mut parts = line.split_whitespace();
		let s = Sample {
			t: parts.next()?.parse().ok()?,
			signal: parts.next()?.parse().ok()?,
			current: parts.next()?.parse().ok()?,
		};
		if s.is_finite() {
			Some(s)
		} else {
			None
		}
	}

	fn write(&self, out: &mut impl Write) -> io::Result<()> {
		if !self.is_finite() {
			return Ok(());
		}
		writeln!(out, "{:.9} {:+.6} {:+.6}", self.t, self.signal, self.current)
	}
}

// ── Core stages (used by toolpipe) ─────────────────────────────────────────

fn ground(sample_rate: f64, num_samples: usize) {
	if !sample_rate.is_finite() || sample_rate <= 0.0 {
		eprintln!("invalid sample_rate: {sample_rate}");
		return;
	}
	let mut out = io::stdout().lock();
	for i in 0..num_samples {
		let s = Sample {
			t: i as f64 / sample_rate,
			signal: 0.0,
			current: 0.0,
		};
		if s.is_finite() && s.write(&mut out).is_err() {
			break;
		}
	}
}

fn ac_source(freq_hz: f64, amplitude: f64) {
	if !freq_hz.is_finite() || !amplitude.is_finite() {
		eprintln!("invalid ac_source parameters");
		return;
	}
	let stdin = io::stdin();
	let mut out = io::stdout().lock();
	for line in stdin.lock().lines() {
		let Ok(line) = line else { break };
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
		if s.is_finite() && s.write(&mut out).is_err() {
			break;
		}
	}
}

fn capacitor(capacitance_f: f64, dt: f64) {
	if !capacitance_f.is_finite() || !dt.is_finite() || dt <= 0.0 {
		eprintln!("invalid capacitor parameters");
		return;
	}
	let stdin = io::stdin();
	let mut out = io::stdout().lock();
	let mut prev = 0.0;
	let mut dc = 0.0;
	let alpha = 0.05;
	for line in stdin.lock().lines() {
		let Ok(line) = line else { break };
		if line.trim().is_empty() {
			continue;
		}
		let mut s = match Sample::from_line(&line) {
			Some(s) => s,
			None => continue,
		};
		dc = (1.0 - alpha) * dc + alpha * s.signal;
		let dv = s.signal - prev;
		let i = capacitance_f * (dv / dt);
		prev = s.signal;
		s.signal -= dc;
		s.current = i;
		if s.is_finite() && s.write(&mut out).is_err() {
			break;
		}
	}
}

fn inductor(inductance_h: f64, dt: f64) {
	if !inductance_h.is_finite() || inductance_h == 0.0 || !dt.is_finite() || dt <= 0.0 {
		eprintln!("invalid inductor parameters");
		return;
	}
	let stdin = io::stdin();
	let mut out = io::stdout().lock();
	let mut flux = 0.0;
	for line in stdin.lock().lines() {
		let Ok(line) = line else { break };
		if line.trim().is_empty() {
			continue;
		}
		let mut s = match Sample::from_line(&line) {
			Some(s) => s,
			None => continue,
		};
		flux += (s.signal / inductance_h) * dt * 1e6;
		s.current = flux;
		if s.is_finite() && s.write(&mut out).is_err() {
			break;
		}
	}
}

// ── toolpipe: pure linear cascade (Unix-style) ──────────────────────────────

fn run_pipe_stage(args: &[String]) {
	if args.is_empty() {
		eprintln!("toolpipe stages: ground | ac | cap | ind");
		process::exit(1);
	}
	match args[0].as_str() {
		"ground" => {
			if args.len() != 3 {
				usage_pipe();
			}
			let rate: f64 = args[1].parse().unwrap_or_else(|_| usage_pipe());
			let n: usize = args[2].parse().unwrap_or_else(|_| usage_pipe());
			ground(rate, n);
		}
		"ac" => {
			if args.len() != 3 {
				usage_pipe();
			}
			let f: f64 = args[1].parse().unwrap_or_else(|_| usage_pipe());
			let a: f64 = args[2].parse().unwrap_or_else(|_| usage_pipe());
			ac_source(f, a);
		}
		"cap" => {
			if args.len() != 3 {
				usage_pipe();
			}
			let c: f64 = args[1].parse().unwrap_or_else(|_| usage_pipe());
			let dt: f64 = args[2].parse().unwrap_or_else(|_| usage_pipe());
			capacitor(c, dt);
		}
		"ind" => {
			if args.len() != 3 {
				usage_pipe();
			}
			let l: f64 = args[1].parse().unwrap_or_else(|_| usage_pipe());
			let dt: f64 = args[2].parse().unwrap_or_else(|_| usage_pipe());
			inductor(l, dt);
		}
		_ => usage_pipe(),
	}
}

fn usage_pipe() -> ! {
	eprintln!(
		"toolpipe – linear stage cascade

  toolpipe ground <rate> <n>
  toolpipe ac     <freq_hz> <amp>
  toolpipe cap    <C_F> <dt>
  toolpipe ind    <L_H> <dt>

Example (matches schematic values):
  toolpipe ground 200000 40 \\
	| toolpipe ac 22000 5 \\
	| toolpipe ind 470e-6 5e-6 \\
	| toolpipe cap 1e-6 5e-6 \\
	| toolpipe ac 22000 3 \\
	| toolpipe ind 470e-6 5e-6 \\
	| toolpipe cap 1e-6 5e-6"
	);
	process::exit(1);
}

// ── tooltree: branched approximation of the schematic ───────────────────────

fn run_tree(args: &[String]) {
	let sample_rate: f64 = args.get(0).and_then(|s| s.parse().ok()).unwrap_or(200_000.0);
	let num_samples: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(40);
	if !sample_rate.is_finite() || sample_rate <= 0.0 {
		eprintln!("invalid sample_rate: {sample_rate}");
		return;
	}
	let dt = 1.0 / sample_rate;
	if !dt.is_finite() || dt <= 0.0 {
		eprintln!("invalid dt: {dt}");
		return;
	}

	let l1 = 470e-6;
	let c1 = 1e-6;
	let l2 = 470e-6;
	let c2 = 1e-6;
	let f = 22_000.0;

	let mut out = io::stdout().lock();

	let mut prev1 = 0.0;
	let mut dc1 = 0.0;
	let mut flux1 = 0.0;
	let mut prev2 = 0.0;
	let mut dc2 = 0.0;
	let mut flux2 = 0.0;
	let alpha = 0.05;

	for i in 0..num_samples {
		let t = i as f64 / sample_rate;

		let drive_a = 5.0 * (2.0 * PI * f * t).sin();
		flux1 += (drive_a / l1) * dt * 1e6;
		let after_l1 = flux1 * 0.001;
		dc1 = (1.0 - alpha) * dc1 + alpha * after_l1;
		let dv1 = after_l1 - prev1;
		let i_c1 = c1 * (dv1 / dt);
		prev1 = after_l1;
		let sig_a = after_l1 - dc1;

		let drive_b = 3.0 * (2.0 * PI * f * t).sin();
		flux2 += (drive_b / l2) * dt * 1e6;
		let after_l2 = flux2 * 0.001;
		dc2 = (1.0 - alpha) * dc2 + alpha * after_l2;
		let dv2 = after_l2 - prev2;
		let i_c2 = c2 * (dv2 / dt);
		prev2 = after_l2;
		let sig_b = after_l2 - dc2;

		let signal = sig_a * 0.6 + sig_b * 0.4 + 0.5 * (2.0 * PI * f * t).sin();
		let current = i_c1 + i_c2;

		let s = Sample { t, signal, current };
		if s.is_finite() && s.write(&mut out).is_err() {
			break;
		}
	}
}

fn usage_tree() -> ! {
	eprintln!(
		"tooltree – branched approximation of the schematic

  tooltree [sample_rate] [num_samples]

Defaults: 200000 Hz, 40 samples
Uses the 22 kHz sources, 470 µH inductors and 1 µF capacitors from the drawing.

Example:
  tooltree 200000 50
  tooltree | head
  tooltree 100000 100 > samples.txt"
	);
	process::exit(1);
}

// ── Entry point (tree cancels pipe; pipe cancels tree) ──────────────────────

fn main() {
	let args: Vec<String> = env::args().collect();
	let prog = args[0].rsplit('/').next().unwrap_or("tool");

	// Strictly select one mode so 'tree' cancels 'pipe' and 'pipe' cancels 'tree'
	let mode = if prog == "tooltree" || (args.len() > 1 && args[1] == "tree") {
		"tree"
	} else if prog == "toolpipe" || (args.len() > 1 && args[1] == "pipe") {
		"pipe"
	} else {
		"none"
	};

	match mode {
		"tree" => {
			let tree_args = if prog == "tooltree" { &args[1..] } else { &args[2..] };
			if !tree_args.is_empty() && (tree_args[0] == "-h" || tree_args[0] == "--help") {
				usage_tree();
			}
			run_tree(tree_args);
		}
		"pipe" => {
			let stage_args = if prog == "toolpipe" { &args[1..] } else { &args[2..] };
			run_pipe_stage(stage_args);
		}
		_ => {
			eprintln!(
				"Usage:
  ./tool pipe  <stage args...>     # or install as 'toolpipe' (cancels tree)
  ./tool tree  [rate] [n]          # or install as 'tooltree' (cancels pipe)

  toolpipe  – linear Unix-style stage cascade
  tooltree  – branched model of the schematic

Examples:
  toolpipe ground 200000 30 | toolpipe ac 22000 5 | toolpipe ind 470e-6 5e-6
  tooltree 200000 50
  tooltree | toolpipe cap 1e-6 5e-6"
			);
			process::exit(1);
		}
	}
}
