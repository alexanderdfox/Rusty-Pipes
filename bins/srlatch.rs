// src/main.rs
use std::io::{self, BufRead, Write};

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
	Low = 0,
	High = 1,
}

impl State {
	fn as_str(self) -> &'static str {
		match self {
			State::Low => "0",
			State::High => "1",
		}
	}
}

fn main() {
	let stdin = io::stdin();
	let mut stdout = io::stdout();
	let mut q = State::Low; // power-on default

	for line in stdin.lock().lines() {
		let line = match line {
			Ok(l) => l.trim().to_lowercase(),
			Err(_) => break,
		};

		if line.is_empty() || line.starts_with('#') {
			continue;
		}

		match line.as_str() {
			"s" | "set" | "1" => {
				q = State::High;
			}
			"r" | "reset" | "0" => {
				q = State::Low;
			}
			"t" | "toggle" => {
				q = if q == State::High { State::Low } else { State::High };
			}
			"q" | "status" => {
				// just report, do not change
			}
			"both" | "illegal" => {
				// classic SR forbidden state – we treat it as undefined / keep previous
				// (in real silicon this can oscillate or go metastable)
				let _ = writeln!(stdout, "WARNING: both S and R asserted – state undefined");
				let _ = stdout.flush();
				continue;
			}
			_ => {
				let _ = writeln!(stdout, "unknown command: {}", line);
				let _ = stdout.flush();
				continue;
			}
		}

		// Always emit current state after a command (plumbing “output pipe”)
		let _ = writeln!(stdout, "Q={}", q.as_str());
		let _ = stdout.flush();
	}
}