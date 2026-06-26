use std::fs;
use std::path::Path;

use rover_proto::v1::ServerMetrics;

// --- Public API ---

/// Collect current server metrics: CPU, RAM, disk.
///
/// RAM reads `/proc/meminfo`. Disk uses `df -k`.
/// CPU is stubbed at 0.0 — `/proc/stat` is blocked by SELinux on non-rooted
/// Android, and `top -b -n 1` parsing proved unreliable across toybox versions.
/// A working method will be found later.
pub async fn collect_metrics() -> ServerMetrics {
    let cpu_percent = 0.0;
    let (ram_used, ram_total) = ram_usage();
    let (disk_used, disk_total) = {
        let path = data_dir();
        tokio::task::spawn_blocking(move || disk_usage(&path))
            .await
            .unwrap_or((0, 0))
    };

    ServerMetrics {
        cpu_percent,
        ram_used_bytes: ram_used,
        ram_total_bytes: ram_total,
        disk_used_bytes: disk_used,
        disk_total_bytes: disk_total,
    }
}

// --- RAM collection ---

fn ram_usage() -> (u64, u64) {
    match read_meminfo() {
        Ok((total, available)) => {
            let used = total.saturating_sub(available);
            (used * 1024, total * 1024)
        }
        Err(e) => {
            tracing::warn!("failed to read RAM stats: {e}");
            (0, 0)
        }
    }
}

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

fn parse_kb_value(s: &str) -> Result<u64, String> {
    let num_str = s.split_whitespace().next().ok_or("empty value")?;
    num_str
        .parse::<u64>()
        .map_err(|e| format!("parse meminfo value '{num_str}': {e}"))
}

// --- Disk collection ---

fn disk_usage(path: &Path) -> (u64, u64) {
    let target = if path.exists() {
        path.to_path_buf()
    } else {
        path.ancestors()
            .find(|p| p.exists())
            .unwrap_or_else(|| Path::new("/"))
            .to_path_buf()
    };

    match std::process::Command::new("df")
        .args(["-k", target.to_str().unwrap_or("/")])
        .output()
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().nth(1) {
                let fields: Vec<&str> = line.split_whitespace().collect();
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
    fn test_disk_usage_root() {
        let (_used, total) = disk_usage(Path::new("/"));
        assert!(total > 0, "total disk should be > 0");
    }

    #[test]
    fn test_disk_usage_nonexistent_falls_back() {
        let (_used, total) = disk_usage(Path::new("/nonexistent/path/xyzzy"));
        assert!(total > 0, "should fall back to existing ancestor");
    }
}
