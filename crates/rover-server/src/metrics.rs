use std::fs;
use std::path::Path;

use rover_proto::v1::ServerMetrics;

// --- Public API ---

/// Collect current server metrics: CPU, RAM, disk.
///
/// RAM and disk use direct reads for Termux/Android compatibility.
/// CPU is stubbed at 0.0 — `/proc/stat` and `/proc/uptime` are blocked
/// by SELinux on non-rooted Android. A working method will be found later.
pub fn collect_metrics() -> ServerMetrics {
    let cpu_percent = 0.0;
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
    fn test_collect_metrics_returns_zero_cpu() {
        let metrics = collect_metrics();
        assert_eq!(metrics.cpu_percent, 0.0);
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
