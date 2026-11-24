#!/usr/bin/env bash
# tiny8.sh – Minimal 8-bit computer built from the flip-flop idea
# Every register bit is conceptually an SR latch (set/reset memory)

set -euo pipefail

# ------------------------------------------------------------
# 8-bit registers (each bit is a flip-flop)
# ------------------------------------------------------------
declare -i A=0          # Accumulator
declare -i PC=0         # Program Counter
declare -a RAM=(0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0)  # 16 bytes

# ------------------------------------------------------------
# Helper: keep everything 8-bit
# ------------------------------------------------------------
mask8() { echo $(($1 & 0xFF)); }

# ------------------------------------------------------------
# Instruction set (very reduced)
# High nibble = opcode, low nibble = address / immediate
# ------------------------------------------------------------
# 0x0n  LOAD  n     A ← RAM[n]
# 0x1n  STORE n     RAM[n] ← A
# 0x2n  ADD   n     A ← A + RAM[n]
# 0x3n  SUB   n     A ← A - RAM[n]
# 0x4n  JMP   n     PC ← n
# 0x5n  JZ    n     if A == 0 then PC ← n
# 0x6n  LDAI  n     A ← n          (load immediate)
# 0xFF  HLT
# ------------------------------------------------------------

disasm() {
	local op=$(( $1 >> 4 ))
	local arg=$(( $1 & 0x0F ))
	case $op in
		0) echo "LOAD  $arg" ;;
		1) echo "STORE $arg" ;;
		2) echo "ADD   $arg" ;;
		3) echo "SUB   $arg" ;;
		4) echo "JMP   $arg" ;;
		5) echo "JZ    $arg" ;;
		6) echo "LDAI  $arg" ;;
		15) echo "HLT" ;;
		*) echo "???" ;;
	esac
}

# ------------------------------------------------------------
# Load a tiny demo program into RAM
# This program: A = 7 + 5 → store result in RAM[2] → halt
# ------------------------------------------------------------
load_demo() {
	# address : instruction
	RAM[0]=0x67   # LDAI 7
	RAM[1]=0x25   # ADD   5     (we put 5 in RAM[5] below)
	RAM[2]=0x12   # STORE 2
	RAM[3]=0xFF   # HLT

	RAM[5]=5      # data: the number 5
}

# ------------------------------------------------------------
# One clock cycle: fetch → decode → execute
# ------------------------------------------------------------
clock() {
	local instr=${RAM[$PC]}
	local op=$(( instr >> 4 ))
	local arg=$(( instr & 0x0F ))

	printf "PC=%02X  IR=%02X  %-8s  A=%3d  " "$PC" "$instr" "$(disasm $instr)" "$A"

	case $op in
		0)  # LOAD
			A=$(mask8 ${RAM[$arg]})
			PC=$((PC + 1))
			;;
		1)  # STORE
			RAM[$arg]=$(mask8 $A)
			PC=$((PC + 1))
			;;
		2)  # ADD
			A=$(mask8 $((A + RAM[$arg])))
			PC=$((PC + 1))
			;;
		3)  # SUB
			A=$(mask8 $((A - RAM[$arg])))
			PC=$((PC + 1))
			;;
		4)  # JMP
			PC=$arg
			;;
		5)  # JZ
			if (( A == 0 )); then
				PC=$arg
			else
				PC=$((PC + 1))
			fi
			;;
		6)  # LDAI (load immediate)
			A=$(mask8 $arg)
			PC=$((PC + 1))
			;;
		15) # HLT
			echo "→ HALT"
			return 1
			;;
		*)
			echo "→ ILLEGAL"
			return 1
			;;
	esac

	echo "→ A=$A"
	return 0
}

# ------------------------------------------------------------
# Run the machine
# ------------------------------------------------------------
echo "=== Tiny 8-bit Computer (flip-flop based registers) ==="
echo

load_demo

echo "Program loaded:"
for i in {0..5}; do
	printf "  RAM[%2d] = %02X  %s\n" "$i" "${RAM[$i]}" "$(disasm ${RAM[$i]})"
done
echo

echo "Running..."
echo "----------------------------------------"

while true; do
	if ! clock; then
		break
	fi
	# Uncomment for single-step:
	# read -p "Press enter for next clock..."
done

echo "----------------------------------------"
echo "Final state:"
echo "  Accumulator A = $A"
echo "  RAM[2]        = ${RAM[2]}   (should be 12)"
echo
echo "Done."