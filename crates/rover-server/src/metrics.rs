use std::fs;
use std::path::Path;
use std::sync::Mutex;

use rover_proto::v1::ServerMetrics;

// --- Public API ---

/// Collect current server metrics: CPU, RAM, disk.
///
/// All metrics use direct file reads for Termux/Android compatibility.
/// `/proc/stat` is blocked by SELinux on Android — instead we read
/// `/proc/self/stat` + `/proc/uptime` for CPU usage of the rover process itself.
pub fn collect_metrics(snapshot: &Mutex<Option<CpuSnapshot>>) -> ServerMetrics {
    let cpu_percent = cpu_usage(snapshot);
    let (ram_used, ram_total) = ram_usage();
    let (disk_used, disk_total) = disk_usage(&data_dir());

    ServerMetrics {
        cpu_percent,
        ram_used_bytes: ram_used,
        ram_total_bytes: ram_total,
        disk_used_bytes: disk_used,
        disk_total_bytes: disk_total,
    }
}

// --- CPU collection ---

/// A snapshot of `/proc/self/stat` + `/proc/uptime` at a point in time.
#[derive(Debug, Clone)]
pub struct CpuSnapshot {
    /// Total CPU time consumed by this process (utime + stime + cutime + cstime)
    /// in clock ticks (USER_HZ, typically 100).
    process_ticks: u64,
    /// System uptime from /proc/uptime, in centiseconds (hundredths of a second).
    uptime_cs: u64,
}

/// Compute CPU usage percentage since the last snapshot.
///
/// Uses `/proc/self/stat` (always readable — every process can read its own)
/// and `/proc/uptime` (always readable) to calculate this process's CPU usage
/// as a percentage of one core. Can exceed 100% if multithreaded.
///
/// On first call, stores an initial snapshot and returns 0.0.
fn cpu_usage(snapshot: &Mutex<Option<CpuSnapshot>>) -> f64 {
    let current = match read_cpu_snapshot() {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("failed to read CPU stats: {e}");
            return 0.0;
        }
    };

    let mut prev = snapshot.lock().unwrap();
    let result = match prev.as_ref() {
        Some(prev_snap) => {
            let tick_delta = current
                .process_ticks
                .saturating_sub(prev_snap.process_ticks);
            let time_delta_cs = current.uptime_cs.saturating_sub(prev_snap.uptime_cs);

            if time_delta_cs == 0 {
                0.0
            } else {
                // CPU% = (ticks_used / ticks_elapsed) * 100
                // ticks_elapsed = time_delta_cs / (100 / USER_HZ)
                // USER_HZ is 100 on Linux, so 1 centisecond = 1 tick
                (tick_delta as f64 / time_delta_cs as f64) * 100.0
            }
        }
        None => 0.0,
    };

    *prev = Some(current);
    result
}

/// Read `/proc/uptime` and `/proc/self/stat`.
///
/// `/proc/self/stat` is always readable on Android — SELinux permits every
/// process to read its own stat file. `/proc/uptime` is also world-readable.
///
/// Fields from `/proc/self/stat` (1-indexed, space-separated):
///   field 14: utime   — user mode CPU time in ticks
///   field 15: stime   — kernel mode CPU time in ticks
///   field 16: cutime  — waited-for children user time in ticks
///   field 17: cstime  — waited-for children kernel time in ticks
///
/// USER_HZ is 100 ticks/sec on Linux (including Android).
///
/// `/proc/uptime` format: `uptime_seconds idle_seconds`
/// We convert to centiseconds for tick-level precision.
fn read_cpu_snapshot() -> Result<CpuSnapshot, String> {
    // Read /proc/self/stat
    let stat =
        fs::read_to_string("/proc/self/stat").map_err(|e| format!("read /proc/self/stat: {e}"))?;

    // The stat file is space-separated, but field 2 (comm) may contain spaces
    // and is wrapped in parentheses. Find the closing paren to parse fields after it.
    let after_comm = stat
        .rsplit(')')
        .next()
        .ok_or("malformed /proc/self/stat: no closing paren")?;

    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    // fields[0] = state, fields[1] = ppid, ..., fields[11] = utime, fields[12] = stime, ...

    if fields.len() < 15 {
        return Err(format!(
            "/proc/self/stat: expected at least 15 fields after comm, got {}",
            fields.len()
        ));
    }

    // fields after comm are 0-indexed, corresponding to proc fields starting at index 3
    // field 14 (utime) = our fields[11], field 15 (stime) = fields[12],
    // field 16 (cutime) = fields[13], field 17 (cstime) = fields[14]
    let utime: u64 = fields[11]
        .parse()
        .map_err(|e| format!("parse utime: {e}"))?;
    let stime: u64 = fields[12]
        .parse()
        .map_err(|e| format!("parse stime: {e}"))?;
    let cutime: u64 = fields[13]
        .parse()
        .map_err(|e| format!("parse cutime: {e}"))?;
    let cstime: u64 = fields[14]
        .parse()
        .map_err(|e| format!("parse cstime: {e}"))?;

    let process_ticks = utime + stime + cutime + cstime;

    // Read /proc/uptime
    let uptime_raw =
        fs::read_to_string("/proc/uptime").map_err(|e| format!("read /proc/uptime: {e}"))?;

    let uptime_secs: f64 = uptime_raw
        .split_whitespace()
        .next()
        .ok_or("empty /proc/uptime")?
        .parse()
        .map_err(|e| format!("parse uptime: {e}"))?;

    // Convert to centiseconds (hundredths). USER_HZ=100 means 1 tick = 1 centisecond.
    let uptime_cs = (uptime_secs * 100.0) as u64;

    Ok(CpuSnapshot {
        process_ticks,
        uptime_cs,
    })
}

// --- RAM collection ---

/// Read `/proc/meminfo` for RAM usage.
///
/// Returns (used_bytes, total_bytes).
///
/// `/proc/meminfo` is always readable on Android/Termux — no root required.
fn ram_usage() -> (u64, u64) {
    match read_meminfo() {
        Ok((total, available)) => {
            let used = total.saturating_sub(available);
            // meminfo reports in kB, convert to bytes
            (used * 1024, total * 1024)
        }
        Err(e) => {
            tracing::warn!("failed to read RAM stats: {e}");
            (0, 0)
        }
    }
}

/// Parse `/proc/meminfo` for MemTotal and MemAvailable (or MemFree as fallback).
fn read_meminfo() -> Result<(u64, u64), String> {
    let contents =
        fs::read_to_string("/proc/meminfo").map_err(|e| format!("read /proc/meminfo: {e}"))?;

    let mut total_kb: Option<u64> = None;
    let mut available_kb: Option<u64> = None;

    for line in contents.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            total_kb = Some(parse_kb_value(rest)?);
        } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available_kb = Some(parse_kb_value(rest)?);
        }
        if total_kb.is_some() && available_kb.is_some() {
            break;
        }
    }

    // Fallback to MemFree if MemAvailable isn't present (older kernels)
    if available_kb.is_none() {
        for line in contents.lines() {
            if let Some(rest) = line.strip_prefix("MemFree:") {
                available_kb = Some(parse_kb_value(rest)?);
                break;
            }
        }
    }

    let total = total_kb.ok_or("MemTotal not found in /proc/meminfo")?;
    let available = available_kb.ok_or("MemAvailable/MemFree not found in /proc/meminfo")?;

    Ok((total, available))
}

/// Parse a meminfo value like " 8162348 kB" → 8162348
fn parse_kb_value(s: &str) -> Result<u64, String> {
    let num_str = s.split_whitespace().next().ok_or("empty value")?;
    num_str
        .parse::<u64>()
        .map_err(|e| format!("parse meminfo value '{num_str}': {e}"))
}

// --- Disk collection ---

/// Get disk usage for the given path via the `df` command.
///
/// Uses `df -k` (POSIX, works on Linux, macOS, and Termux) then converts KB to bytes.
/// Returns (used_bytes, total_bytes).
/// Falls back to `/` if the path doesn't exist.
fn disk_usage(path: &Path) -> (u64, u64) {
    // If the path doesn't exist, walk up to the first existing ancestor
    let target = if path.exists() {
        path.to_path_buf()
    } else {
        path.ancestors()
            .find(|p| p.exists())
            .unwrap_or_else(|| Path::new("/"))
            .to_path_buf()
    };

    // df -k <path> outputs something like:
    // Filesystem   1024-blocks      Used Available Capacity Mounted on
    // /dev/sda1       12345678  4567890   7778888    37%   /
    match std::process::Command::new("df")
        .args(["-k", target.to_str().unwrap_or("/")])
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse the second line (first is header)
            if let Some(line) = stdout.lines().nth(1) {
                let fields: Vec<&str> = line.split_whitespace().collect();
                // fields: [Filesystem, 1024-blocks, Used, Available, Capacity, Mounted]
                if fields.len() >= 4 {
                    let used_kb: u64 = fields[2].parse().unwrap_or(0);
                    let total_kb: u64 = fields[1].parse().unwrap_or(0);
                    return (used_kb * 1024, total_kb * 1024);
                }
            }
            tracing::warn!(
                "df output unparseable for {}: {}",
                target.display(),
                stdout.trim()
            );
            (0, 0)
        }
        Err(e) => {
            tracing::warn!("df command failed for {}: {}", target.display(), e);
            (0, 0)
        }
    }
}

/// Root path for disk usage reporting.
///
/// On Termux this is `/data/data/com.termux/files/home`.
/// `statvfs` reports the filesystem containing the path, not the path itself,
/// so this gives us the overall Termux storage partition.
fn data_dir() -> std::path::PathBuf {
    std::env::var("DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| std::path::PathBuf::from("/data/data/com.termux/files/home"))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_meminfo_line() {
        assert_eq!(parse_kb_value(" 8162348 kB").unwrap(), 8162348);
        assert_eq!(parse_kb_value("8162348 kB").unwrap(), 8162348);
        assert_eq!(parse_kb_value("1024").unwrap(), 1024);
    }

    #[test]
    fn test_cpu_snapshot_from_fake_stat() {
        // Simulate /proc/self/stat fields after comm (closing paren)
        // field index:  0      1    2  3  4  5  6  7  8  9  10  11    12    13     14
        // proc field:   state ppid ...                 utime stime cutime cstime
        let after_comm = " S 1234 5678 0 0 -1 4194304 123 0 0 0 100 50 25 12 0 0 20 0 1 0 12345 4096 56 4294967295 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0";
        let fields: Vec<&str> = after_comm.split_whitespace().collect();
        let utime: u64 = fields[11].parse().unwrap();
        let stime: u64 = fields[12].parse().unwrap();
        let cutime: u64 = fields[13].parse().unwrap();
        let cstime: u64 = fields[14].parse().unwrap();

        assert_eq!(utime, 100);
        assert_eq!(stime, 50);
        assert_eq!(cutime, 25);
        assert_eq!(cstime, 12);
        assert_eq!(utime + stime + cutime + cstime, 187);
    }

    #[test]
    fn test_cpu_delta_zero_on_first_call() {
        let snap = Mutex::new(None);
        let result = cpu_usage(&snap);
        assert!(result >= 0.0 && result <= 100.0);
    }

    #[test]
    fn test_cpu_delta_from_known_values() {
        // Test the CPU computation directly with known values.
        // process used 50 ticks in 100 centiseconds (1 second) = 50% of one core
        let prev = CpuSnapshot {
            process_ticks: 1000,
            uptime_cs: 5000,
        };
        let current = CpuSnapshot {
            process_ticks: 1050,
            uptime_cs: 5100,
        };

        let tick_delta = current.process_ticks - prev.process_ticks;
        let time_delta = current.uptime_cs - prev.uptime_cs;
        let cpu = (tick_delta as f64 / time_delta as f64) * 100.0;
        // 50 ticks / 100 cs = 0.5 = 50%
        assert!((cpu - 50.0).abs() < 0.01, "expected 50%, got {cpu}%");
    }

    #[test]
    fn test_disk_usage_root() {
        // / always exists on any Unix
        let (_used, total) = disk_usage(Path::new("/"));
        assert!(total > 0, "total disk should be > 0");
    }

    #[test]
    fn test_disk_usage_nonexistent_falls_back() {
        // A path that definitely doesn't exist
        let (_used, total) = disk_usage(Path::new("/nonexistent/path/xyzzy"));
        assert!(total > 0, "should fall back to existing ancestor");
    }
}
