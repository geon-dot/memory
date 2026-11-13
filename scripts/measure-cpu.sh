#!/usr/bin/env bash
# Measures how much CPU a running System Monitor uses on macOS: the app
# process plus the WebKit helper processes that draw its widget, averaged
# over a stretch of time.
#
#   scripts/measure-cpu.sh (--pid <pid> | --app <path/to/X.app>) [--seconds <n>]
#
# It reads each process's accumulated CPU time (user + system, from `ps`) at
# the start and at the end and divides the difference by the time between
# the two readings. 100% is one core busy the whole time. The default is 60 seconds.
#
# The processes are the ones scripts/measure-memory.sh lists (see there for
# how the WebKit helpers are tied to the app). A helper that starts or exits
# in between is counted for the readings it was there for.
set -euo pipefail

seconds=60
pass=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --seconds)
      [[ -n "${2:-}" ]] || { echo "--seconds needs a value" >&2; exit 2; }
      seconds=$2; shift 2 ;;
    --pid|--app)
      [[ -n "${2:-}" ]] || { echo "$1 needs a value" >&2; exit 2; }
      pass+=("$1" "$2"); shift 2 ;;
    -h|--help) sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
if [[ ${#pass[@]} -eq 0 ]]; then
  echo "say which app to measure: --pid <pid> or --app <path/to/X.app>" >&2
  exit 2
fi

here=$(cd "$(dirname "$0")" && pwd)
listing=$("$here/measure-memory.sh" "${pass[@]}")
echo "$listing" | head -1

# "pid name" for the app and each helper, from the memory listing's rows.
procs=$(awk '$1 ~ /^[0-9]+$/ { print $1, $2 }' <<<"$listing")

# Accumulated CPU seconds of a process ("[[dd-]hh:]mm:ss.cc" from ps), empty if it is gone.
cpu_seconds() {
  ps -o cputime= -p "$1" 2>/dev/null | awk '{
    n = split($1, t, /[-:]/); s = 0
    for (i = 1; i <= n; i++) s = s * (i == 2 && n == 4 ? 24 : 60) + t[i]
    printf "%.2f", s
  }'
}

before=""
while read -r p name; do
  before+="$p $name $(cpu_seconds "$p")"$'\n'
done <<<"$procs"
start=$(date +%s)
sleep "$seconds"
elapsed=$(( $(date +%s) - start ))

printf '%-8s %-28s %10s %8s\n' PID PROCESS "CPU TIME" "AVERAGE"
total=0
while read -r p name was; do
  [[ -z "$p" ]] && continue
  now=$(cpu_seconds "$p")
  [[ -z "$now" || -z "$was" ]] && continue
  used=$(awk -v a="$now" -v b="$was" 'BEGIN { printf "%.2f", a - b }')
  printf '%-8s %-28s %8s s %7s%%\n' "$p" "$name" "$used" \
    "$(awk -v u="$used" -v e="$elapsed" 'BEGIN { printf "%.2f", u / e * 100 }')"
  total=$(awk -v a="$total" -v b="$used" 'BEGIN { printf "%.2f", a + b }')
done <<<"$before"
printf '%-8s %-28s %8s s %7s%%\n' "" "TOTAL over ${elapsed} s" "$total" \
  "$(awk -v u="$total" -v e="$elapsed" 'BEGIN { printf "%.2f", u / e * 100 }')"
