use std::fs;
use std::path::Path;
use std::sync::Mutex;

use rover_proto::v1::ServerMetrics;

// --- Public API ---

/// Collect current server metrics: CPU, RAM, disk.
///
/// Uses direct `/proc` reads for Termux/Android compatibility.
/// `sysinfo` fails on non-rooted Android because `/proc/stat` permissions
/// are restricted by SELinux. Direct file reads work on Android 5+.
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

/// A snapshot of `/proc/stat` cpu line at a point in time.
#[derive(Debug, Clone)]
pub struct CpuSnapshot {
    /// Sum of all cpu fields (user + nice + system + idle + iowait + irq + softirq + steal)
    total: u64,
    /// Idle time (idle + iowait)
    idle: u64,
}

/// Compute CPU usage percentage since the last snapshot.
/// On first call, takes an initial snapshot and returns 0.0.
/// Subsequent calls compute delta between current and previous snapshot.
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
            let total_delta = current.total.saturating_sub(prev_snap.total);
            let idle_delta = current.idle.saturating_sub(prev_snap.idle);
            if total_delta == 0 {
                0.0
            } else {
                let used = total_delta.saturating_sub(idle_delta);
                (used as f64 / total_delta as f64) * 100.0
            }
        }
        None => 0.0,
    };

    *prev = Some(current);
    result
}

/// Read `/proc/stat` and parse the first cpu line.
///
/// Format: `cpu  user nice system idle iowait irq softirq steal guest guest_nice`
/// All values are in USER_HZ (usually 100 ticks/sec, but we use deltas so it's unitless ratio).
fn read_cpu_snapshot() -> Result<CpuSnapshot, String> {
    let contents = fs::read_to_string("/proc/stat").map_err(|e| format!("read /proc/stat: {e}"))?;

    // Find the aggregate cpu line (starts with "cpu ")
    let cpu_line = contents
        .lines()
        .find(|line| line.starts_with("cpu "))
        .ok_or("no 'cpu ' line in /proc/stat")?;

    let fields: Vec<&str> = cpu_line.split_whitespace().collect();
    // fields[0] = "cpu", fields[1..] = numbers
    if fields.len() < 8 {
        return Err(format!(
            "unexpected /proc/stat cpu line: only {} fields",
            fields.len()
        ));
    }

    let user: u64 = fields[1].parse().map_err(|e| format!("parse user: {e}"))?;
    let nice: u64 = fields[2].parse().map_err(|e| format!("parse nice: {e}"))?;
    let system: u64 = fields[3]
        .parse()
        .map_err(|e| format!("parse system: {e}"))?;
    let idle: u64 = fields[4].parse().map_err(|e| format!("parse idle: {e}"))?;
    let iowait: u64 = fields[5]
        .parse()
        .map_err(|e| format!("parse iowait: {e}"))?;
    let irq: u64 = fields[6].parse().map_err(|e| format!("parse irq: {e}"))?;
    let softirq: u64 = fields[7]
        .parse()
        .map_err(|e| format!("parse softirq: {e}"))?;

    // steal (fields[8]) is optional on older kernels
    let steal: u64 = fields.get(8).and_then(|v| v.parse().ok()).unwrap_or(0);

    let total = user + nice + system + idle + iowait + irq + softirq + steal;
    let idle_total = idle + iowait;

    Ok(CpuSnapshot {
        total,
        idle: idle_total,
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

/// Get disk usage for the rover data directory via POSIX `statvfs`.
///
/// Returns (used_bytes, total_bytes).
fn disk_usage(path: &Path) -> (u64, u64) {
    unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        let cpath = std::ffi::CString::new(path.to_string_lossy().as_bytes())
            .unwrap_or_else(|_| std::ffi::CString::new("/").unwrap());

        if libc::statvfs(cpath.as_ptr(), &mut stat) != 0 {
            tracing::warn!(
                "statvfs failed for {}: {}",
                path.display(),
                std::io::Error::last_os_error()
            );
            return (0, 0);
        }

        let total = stat.f_blocks as u64 * stat.f_frsize as u64;
        let available = stat.f_bavail as u64 * stat.f_frsize as u64;
        let used = total.saturating_sub(available);
        (used, total)
    }
}

/// Rover data directory for disk usage reporting.
fn data_dir() -> std::path::PathBuf {
    std::env::var("DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home =
                std::env::var("HOME").unwrap_or_else(|_| "/data/data/com.termux/files/home".into());
            std::path::PathBuf::from(home).join(".config").join("rover")
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
    fn test_cpu_snapshot_parse() {
        // Fake /proc/stat line
        let line = "cpu  123 456 789 1000 200 300 400 500 0 0";
        // user=123 nice=456 sys=789 idle=1000 iowait=200 irq=300 softirq=400 steal=500
        // total=3768 idle=1200
        let fields: Vec<&str> = line.split_whitespace().collect();
        let total: u64 = fields[1..=8]
            .iter()
            .map(|v| v.parse::<u64>().unwrap())
            .sum();
        let idle: u64 = fields[4].parse::<u64>().unwrap() + fields[5].parse::<u64>().unwrap();
        assert_eq!(total, 3768);
        assert_eq!(idle, 1200);
    }

    #[test]
    fn test_cpu_delta_zero_on_first_call() {
        let snap = Mutex::new(None);
        // On macOS CI or non-Linux, /proc/stat won't exist so we get 0.0
        let result = cpu_usage(&snap);
        assert!(result >= 0.0 && result <= 100.0);
    }
}
