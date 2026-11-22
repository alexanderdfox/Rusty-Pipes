#!/usr/bin/env bash
set -euo pipefail

BIN_FILE="${BIN_FILE:-./tool}"
WINDOW_SIZE="${1:-100}"          # Rolling number of samples kept on screen
REFRESH_SEC="${2:-0.15}"         # Loop / refresh pause in seconds

# Changing variables: cycle through sample counts (N_LIST) and rates (RATE_LIST)
# Override from CLI/env, e.g.: N_LIST="25 50 100 75" RATE_LIST="100000 200000 400000" bash scope.sh
N_LIST=(${N_LIST:-25 50 80 120 60 30})
RATE_LIST=(${RATE_LIST:-200000 300000 150000})
MODE="${MODE:-tree}"             # tree | pipe

DATA_FILE="$(mktemp)"
ERR_FILE="$(mktemp)"
FRAME_FILE="$(mktemp)"

cleanup() {
  printf '\033[?25h\n'          # Restore cursor visibility
  rm -f "$DATA_FILE" "$ERR_FILE" "$FRAME_FILE" "${DATA_FILE}.tmp"
}
trap cleanup EXIT INT TERM

if [[ ! -x "$BIN_FILE" ]]; then
  echo "Error: executable binary not found at '$BIN_FILE'" >&2
  exit 1
fi

cols() { tput cols 2>/dev/null || echo 100; }
lines() { tput lines 2>/dev/null || echo 30; }

# Hide cursor for cleaner terminal updates
printf '\033[?25l'

iter=0
while true; do
  n_cur="${N_LIST[$(( iter % ${#N_LIST[@]} ))]}"
  rate_cur="${RATE_LIST[$(( iter % ${#RATE_LIST[@]} ))]}"
  iter=$((iter + 1))

  if [[ "$MODE" == "pipe" ]]; then
	# Example variable sweep into a pipe stage (adjust stage/args to match your build)
	CMD=("$BIN_FILE" pipe ground "$rate_cur" "$n_cur")
  else
	CMD=("$BIN_FILE" tree "$rate_cur" "$n_cur")
  fi

  # Collect numeric lines from this run and append to rolling window
  while IFS= read -r line; do
	nums=$(printf '%s\n' "$line" | grep -Eo -- '-?[0-9]+([.][0-9]+)?([eE][+-]?[0-9]+)?' | head -n 2 | tr '\n' ' ')
	nums="${nums%"${nums##*[![:space:]]}"}"
	if [[ -n "$nums" ]]; then
	  printf '%s\n' "$nums" >> "$DATA_FILE"
	fi
  done < <("${CMD[@]}" 2>>"$ERR_FILE" || true)

  if [[ -s "$DATA_FILE" ]]; then
	tail -n "$WINDOW_SIZE" "$DATA_FILE" > "${DATA_FILE}.tmp" && mv "${DATA_FILE}.tmp" "$DATA_FILE"

	W=$(cols)
	H=$(( $(lines) - 2 ))
	(( H < 10 )) && H=10

	gnuplot -e "
	  set terminal dumb size ${W},${H} ansi;
	  unset key;
	  set title 'iter=${iter} | ${MODE} rate=${rate_cur} n=${n_cur} (win=${WINDOW_SIZE})';
	  set xlabel 'Sample';
	  set ylabel 'Amplitude';
	  set grid;
	  plot '${DATA_FILE}' using 1 with lines
	" > "$FRAME_FILE" 2>/dev/null || true

	if [[ -s "$FRAME_FILE" ]]; then
	  # Home cursor, print full frame, and clear any trailing lines from prior frame
	  printf '\033[H'
	  cat "$FRAME_FILE"
	  printf '\033[J'
	fi
  fi

  sleep "$REFRESH_SEC"
done