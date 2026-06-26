#!/data/data/com.termux/files/usr/bin/bash
# cpu_metric_probe.sh
# Run: chmod +x cpu_metric_probe.sh && ./cpu_metric_probe.sh

INTERVAL="${1:-1}"

have() { command -v "$1" >/dev/null 2>&1; }

section() {
  echo
  echo "=================================================="
  echo "$1"
  echo "=================================================="
}

run_test() {
  local name="$1"
  shift
  section "$name"
  "$@" 2>&1 | head -n 25
}

proc_stat_usage() {
  read -r _ u1 n1 s1 i1 io1 irq1 sirq1 steal1 _ < /proc/stat || return 1
  idle1=$((i1 + io1))
  total1=$((u1 + n1 + s1 + i1 + io1 + irq1 + sirq1 + steal1))

  sleep "$INTERVAL"

  read -r _ u2 n2 s2 i2 io2 irq2 sirq2 steal2 _ < /proc/stat || return 1
  idle2=$((i2 + io2))
  total2=$((u2 + n2 + s2 + i2 + io2 + irq2 + sirq2 + steal2))

  dt=$((total2 - total1))
  di=$((idle2 - idle1))

  [ "$dt" -gt 0 ] || return 1
  awk -v dt="$dt" -v di="$di" 'BEGIN { printf "CPU usage: %.2f%%\n", 100 * (dt - di) / dt }'
}

proc_stat_per_core() {
  awk '/^cpu[0-9]+ / { print }' /proc/stat
}

proc_loadavg() {
  cat /proc/loadavg
}

proc_pressure_cpu() {
  cat /proc/pressure/cpu
}

top_once() {
  top -b -n 1 2>/dev/null || top -n 1
}

toybox_top() {
  toybox top -b -n 1 2>/dev/null || toybox top -n 1
}

ps_cpu() {
  ps -A -o PID,USER,PCPU,ARGS 2>/dev/null | sort -k3 -nr | head -n 15
}

dumpsys_cpuinfo() {
  dumpsys cpuinfo
}

pid_stat_self() {
  pid="$$"
  echo "Self PID: $pid"
  cat "/proc/$pid/stat"
}

pid_stat_all_sample() {
  for f in /proc/[0-9]*/stat; do
    [ -r "$f" ] || continue
    awk '{ print FILENAME, "pid=" $1, "comm=" $2, "utime=" $14, "stime=" $15 }' "$f"
  done | head -n 20
}

sysfs_freqs() {
  for f in /sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq; do
    [ -r "$f" ] || continue
    echo "$f: $(cat "$f") kHz"
  done
}

termux_battery_temp_context() {
  if have termux-battery-status; then
    termux-battery-status
  else
    echo "termux-battery-status not installed."
    echo "Install with: pkg install termux-api"
    echo "Also install the Termux:API Android app."
  fi
}

echo "CPU metric probe for Termux / Android"
echo "Sampling interval: ${INTERVAL}s"

run_test "1. /proc/stat total CPU usage delta" proc_stat_usage
run_test "2. /proc/stat per-core raw counters" proc_stat_per_core
run_test "3. /proc/loadavg load average" proc_loadavg
run_test "4. /proc/pressure/cpu CPU pressure stall info" proc_pressure_cpu
run_test "5. top batch output" top_once
run_test "6. toybox top output" toybox_top
run_test "7. ps process %CPU ranking" ps_cpu
run_test "8. dumpsys cpuinfo Android service output" dumpsys_cpuinfo
run_test "9. /proc/self/stat process CPU counters" pid_stat_self
run_test "10. /sys CPU frequency readings" sysfs_freqs

section "Bonus: process stat sample"
pid_stat_all_sample

section "Bonus: Termux:API thermal/battery context"
termux_battery_temp_context

echo
echo "Done."
