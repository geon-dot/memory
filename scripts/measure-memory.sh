#!/usr/bin/env bash
# Measures how much memory a running System Monitor uses on macOS: the app
# process itself plus the WebKit helper processes that draw its widget.
#
#   scripts/measure-memory.sh (--pid <pid> | --app <path/to/X.app>) [--settle <seconds>] [--label <text>]
#
# The app is picked by PID or by the bundle it was started from, never by
# process name: several builds (an installed copy, a candidate build with
# another identifier) can run side by side with the same process name.
#
# It measures whatever state the app is in right now and prints that state
# (widget shown / hidden), so run it once with the widget open and once with it
# closed. --settle waits before measuring (memory and helper processes take a
# few seconds to settle after the widget opens or closes).
#
# What is counted
#   `footprint`'s phys_footprint for each process: the same number Activity
#   Monitor shows as "Memory" (dirty + compressed + graphics memory the process
#   owns). RSS is printed next to it for reference only: it counts shared
#   framework pages in every process and leaves out GPU memory, which is most
#   of what the WebKit GPU process holds.
#
# Which WebKit helpers belong to this app
#   WebKit's WebContent / GPU / Networking processes are XPC services. Their
#   parent PID is launchd (1), so `ps` cannot tie them to an app, and every
#   other app with a web view (Mail, Slack, ...) runs the same binaries. macOS
#   does record a "responsible process" for every XPC service: the app that
#   asked for it, which is also what Activity Monitor and the TCC privacy
#   prompts attribute the helper to. We read it with
#   responsibility_get_pid_responsible_for_pid() from libSystem and keep the
#   com.apple.WebKit.* processes whose responsible PID is System Monitor's.
#
# Widget shown or hidden
#   From CGWindowListCopyWindowInfo, which needs no screen-recording or
#   accessibility permission: the widget counts as shown when the app has an
#   on-screen window taller than the menu bar (its menu bar item is a window
#   too, but only menu-bar tall). A widget left on another Space, or on a
#   display that is off, is not on screen and so reads as hidden: bring it to
#   the current Space before measuring the shown state. The number of WebKit
#   helpers in the TOTAL line is the other tell: hidden widgets of builds that
#   release the web view have no WebContent helper.
#
# Needs the Xcode command line tools (`swift`, already needed to build the app).
set -euo pipefail

APP_NAME=system-monitor
pid=""
app=""
settle=0
label=""

value() {
  if [[ -z "${2:-}" ]]; then
    echo "$1 needs a value" >&2
    exit 2
  fi
  printf '%s' "$2"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --pid) pid=$(value "$@"); shift 2 ;;
    --app) app=$(value "$@"); shift 2 ;;
    --settle) settle=$(value "$@"); shift 2 ;;
    --label) label=$(value "$@"); shift 2 ;;
    -h|--help) sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ "$(uname)" != "Darwin" ]]; then
  echo "macOS only" >&2
  exit 2
fi

if [[ -z "$pid" && -z "$app" ]]; then
  echo "say which app to measure: --pid <pid> or --app <path/to/X.app>" >&2
  exit 2
fi

if [[ -z "$pid" ]]; then
  exe="$(cd "$app" && pwd -P)/Contents/MacOS/$APP_NAME"
  # Exact match on the executable path; `ps` prints it as the process was started.
  pids=$(ps -axo pid=,command= | awk -v exe="$exe" '{ p = $1; sub(/^ *[0-9]+ /, ""); if ($0 == exe) print p }')
  count=$(printf '%s' "$pids" | grep -c . || true)
  if [[ "$count" -eq 0 ]]; then
    echo "not running: $exe" >&2
    exit 1
  elif [[ "$count" -gt 1 ]]; then
    echo "several processes run $exe; pick one with --pid: $pids" >&2
    exit 1
  fi
  pid=$pids
elif ! ps -p "$pid" >/dev/null; then
  echo "no process $pid" >&2
  exit 1
fi

if [[ "$settle" != "0" ]]; then
  sleep "$settle"
fi

probe=$(mktemp -t measure-memory).swift
trap 'rm -f "$probe"' EXIT
cat > "$probe" <<'SWIFT'
import CoreGraphics
import Foundation

let app = pid_t(CommandLine.arguments[1])!

// Private but stable libSystem symbol (used by Activity Monitor and TCC).
typealias Responsible = @convention(c) (pid_t) -> pid_t
guard let sym = dlsym(UnsafeMutableRawPointer(bitPattern: -2), // RTLD_DEFAULT
                      "responsibility_get_pid_responsible_for_pid") else {
    FileHandle.standardError.write("responsibility API not found\n".data(using: .utf8)!)
    exit(1)
}
let responsible = unsafeBitCast(sym, to: Responsible.self)

// Every process's executable path, from `ps`.
let ps = Process()
ps.executableURL = URL(fileURLWithPath: "/bin/ps")
ps.arguments = ["-axo", "pid=,comm="]
let pipe = Pipe()
ps.standardOutput = pipe
try! ps.run()
let out = String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)!
ps.waitUntilExit()

for line in out.split(separator: "\n") {
    let parts = line.trimmingCharacters(in: .whitespaces).split(separator: " ", maxSplits: 1)
    guard parts.count == 2, let p = pid_t(parts[0]), p != app else { continue }
    let name = (String(parts[1]) as NSString).lastPathComponent
    if name.hasPrefix("com.apple.WebKit.") && responsible(p) == app {
        print("helper \(p) \(name)")
    }
}

let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID)
    as? [[String: Any]] ?? []
var shown = 0
for w in windows where (w[kCGWindowOwnerPID as String] as? pid_t) == app {
    let b = w[kCGWindowBounds as String] as? [String: Any]
    if let h = b?["Height"] as? Double, h > 40 { shown += 1 }
}
print("windows \(shown)")
SWIFT

probe_out=$(swift "$probe" "$pid")

footprint_mib() {
  # phys_footprint in bytes -> MiB with one decimal. Empty if the process is gone.
  footprint --noCategories -f bytes -p "$1" 2>/dev/null |
    awk '/phys_footprint:/ { printf "%.1f", $2 / 1048576; exit }'
}

rss_mib() {
  ps -o rss= -p "$1" 2>/dev/null | awk '{ printf "%.1f", $1 / 1024 }'
}

windows=$(awk '/^windows / { print $2 }' <<<"$probe_out")
if [[ "${windows:-0}" -gt 0 ]]; then state="shown"; else state="hidden"; fi

echo "System Monitor pid $pid  widget: $state${label:+  ($label)}"
printf '%-8s %-28s %12s %10s\n' PID PROCESS "FOOTPRINT" "RSS"

total_fp=0
total_rss=0
row() {
  local p="$1" name="$2" fp rss
  fp=$(footprint_mib "$p")
  rss=$(rss_mib "$p")
  [[ -z "$fp" ]] && return 0 # exited between listing and measuring
  printf '%-8s %-28s %8s MiB %6s MiB\n' "$p" "$name" "$fp" "$rss"
  total_fp=$(awk -v a="$total_fp" -v b="$fp" 'BEGIN { printf "%.1f", a + b }')
  total_rss=$(awk -v a="$total_rss" -v b="$rss" 'BEGIN { printf "%.1f", a + b }')
}

row "$pid" "$APP_NAME"
helpers=0
while read -r _ hp hname; do
  row "$hp" "$hname"
  helpers=$((helpers + 1))
done < <(grep '^helper ' <<<"$probe_out" || true)

printf '%-8s %-28s %8s MiB %6s MiB\n' "" "TOTAL ($helpers WebKit helpers)" "$total_fp" "$total_rss"
