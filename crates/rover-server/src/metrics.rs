use std::fs;
use std::path::Path;

use rover_proto::v1::ServerMetrics;

// --- Public API ---

/// Collect current server metrics: CPU, RAM, disk.
///
/// CPU uses `top -b -n 1` (available in Termux/toybox without root).
/// RAM reads `/proc/meminfo`.
/// Disk uses `df -k`.
///
/// All subprocess calls run on tokio's blocking thread pool via
/// `spawn_blocking` to avoid stalling the async runtime.
pub async fn collect_metrics() -> ServerMetrics {
    let (cpu_percent, (ram_used, ram_total), (disk_used, disk_total)) =
        tokio::join!(cpu_usage(), async { ram_usage() }, async {
            let path = data_dir();
            tokio::task::spawn_blocking(move || disk_usage(&path))
                .await
                .unwrap_or((0, 0))
        },);

    ServerMetrics {
        cpu_percent,
        ram_used_bytes: ram_used,
        ram_total_bytes: ram_total,
        disk_used_bytes: disk_used,
        disk_total_bytes: disk_total,
    }
}

// --- CPU collection ---

/// Collect system-wide CPU usage percentage via `top -b -n 1`.
///
/// `top` is available on Termux via toybox. On non-rooted Android it's
/// the only way to get system-wide CPU stats — `/proc/stat` and
/// `/proc/loadavg` are blocked by SELinux.
///
/// Runs on the tokio blocking thread pool since `top` takes ~100-300ms.
async fn cpu_usage() -> f64 {
    let result = tokio::task::spawn_blocking(|| -> Result<f64, String> {
        // Try GNU top flags first (-b -n 1), fall back to toybox top flags
        let output = std::process::Command::new("top")
            .args(["-b", "-n", "1"])
            .output()
            .or_else(|_| {
                // Toybox top uses different flags: -n 1 alone for one iteration
                std::process::Command::new("top").args(["-n", "1"]).output()
            })
            .map_err(|e| format!("top command failed: {e}"))?;

        tracing::debug!(
            "top exited with status {:?}, stdout len: {}, stderr len: {}",
            output.status.code(),
            output.stdout.len(),
            output.stderr.len()
        );

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("top exited with error: {}", stderr.trim()));
        }

        parse_top_cpu_header(&output.stdout)
    })
    .await;

    match result {
        Ok(Ok(cpu)) => cpu,
        Ok(Err(e)) => {
            tracing::warn!("failed to get CPU usage: {e}");
            0.0
        }
        Err(e) => {
            tracing::warn!("spawn_blocking panicked: {e}");
            0.0
        }
    }
}

/// Parse the CPU header line from `top` output.
///
/// Handles both GNU top and toybox top formats.
///
/// Toybox top (Termux/Android):
/// ```
/// 800%cpu   0%user   0%nice   0%sys 800%idle   0%iow   0%irq   0%sirq   0%host
/// ```
/// Usage = (total - idle) / total * 100
///
/// GNU top:
/// ```
/// %Cpu(s):  0.0 us,  0.0 sy,  0.0 ni,100.0 id,  0.0 wa,  0.0 hi,  0.0 si,  0.0 st
/// ```
/// Usage = 100 - idle
fn parse_top_cpu_header(stdout: &[u8]) -> Result<f64, String> {
    let text = String::from_utf8_lossy(stdout);

    // Try toybox format: line containing "%cpu" and "%idle"
    if let Some(cpu_line) = text
        .lines()
        .find(|l| l.contains("%cpu") && l.contains("%idle"))
    {
        let total: f64 = cpu_line
            .split_whitespace()
            .find(|f| f.ends_with("%cpu"))
            .and_then(|f| f.trim_end_matches("%cpu").parse().ok())
            .ok_or_else(|| format!("could not parse total cpu from: {cpu_line}"))?;

        let idle: f64 = cpu_line
            .split_whitespace()
            .find(|f| f.ends_with("%idle"))
            .and_then(|f| f.trim_end_matches("%idle").parse().ok())
            .ok_or_else(|| format!("could not parse idle from: {cpu_line}"))?;

        if total <= 0.0 {
            return Err("total CPU capacity is zero".into());
        }

        return Ok(((total - idle) / total) * 100.0);
    }

    // Try GNU top format: line containing "Cpu" and "id,"
    // GNU top outputs: "%Cpu(s):  0.0 us,  0.0 sy,  0.0 ni,100.0 id,  0.0 wa, ..."
    if let Some(cpu_line) = text
        .lines()
        .find(|l| l.contains("Cpu") && l.contains("id,"))
    {
        let idle: f64 = cpu_line
            .split_whitespace()
            .filter(|f| f.starts_with(|c: char| c.is_ascii_digit()) && f.contains("id"))
            .next()
            .and_then(|f| f.trim_end_matches(',').trim_end_matches("id").parse().ok())
            .ok_or_else(|| format!("could not parse idle from: {cpu_line}"))?;

        return Ok((100.0 - idle).max(0.0));
    }

    Err(format!(
        "no recognizable CPU header in top output ({} bytes)",
        stdout.len()
    ))
}

// --- RAM collection ---

/// Read `/proc/meminfo` for RAM usage.
///
/// Returns (used_bytes, total_bytes).
///
/// `/proc/meminfo` is always readable on Android/Termux — no root required.
/// This is a fast sync read, no need for spawn_blocking.
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

/// Get disk usage for the given path via `df -k`.
///
/// Returns (used_bytes, total_bytes).
/// Runs on the tokio blocking thread pool.
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
    fn test_parse_toybox_cpu_idle() {
        // Real output from an 8-core Android phone at near-idle
        let header = "\n\
            Tasks: 7 total,   1 running,   6 sleeping,   0 stopped,   0 zombie\n\
            Mem:    11118M total,     9451M used,     1667M free,        6M buffers\n\
            Swap:     3071M total,      580M used,     2491M free,     5101M cached\n\
            800%cpu   0%user   0%nice   0%sys 800%idle   0%iow   0%irq   0%sirq   0%host\n\
            PID USER         PR  NI VIRT  RES  SHR S[%CPU] %MEM     TIME+ ARGS\n\
            26733 u0_a323      10 -10  10G 4.3M 3.6M R  0.0   0.0   0:00.01 top\n";
        let cpu = parse_top_cpu_header(header.as_bytes()).unwrap();
        assert!((cpu - 0.0).abs() < 0.01, "expected ~0%, got {cpu}%");
    }

    #[test]
    fn test_parse_toybox_cpu_busy() {
        // Simulated busy system: 200% used out of 800% total = 25%
        let header =
            "\n800%cpu  50%user  20%nice  30%sys 600%idle  50%iow  30%irq  20%sirq   0%host\n";
        let cpu = parse_top_cpu_header(header.as_bytes()).unwrap();
        assert!((cpu - 25.0).abs() < 0.5, "expected ~25%, got {cpu}%");
    }

    #[test]
    fn test_parse_toybox_cpu_quad_core() {
        // 4-core device, 50% usage
        let header =
            "400%cpu 100%user  50%nice  50%sys 200%idle  10%iow   0%irq   0%sirq   0%host\n";
        let cpu = parse_top_cpu_header(header.as_bytes()).unwrap();
        assert!((cpu - 50.0).abs() < 0.5, "expected ~50%, got {cpu}%");
    }

    #[test]
    fn test_parse_gnu_cpu_idle() {
        // GNU top format (desktop Linux)
        let header =
            "\n% Cpu(s):  0.0 us,  0.0 sy,  0.0 ni,100.0 id,  0.0 wa,  0.0 hi,  0.0 si,  0.0 st\n";
        let cpu = parse_top_cpu_header(header.as_bytes()).unwrap();
        assert!((cpu - 0.0).abs() < 0.01, "expected ~0%, got {cpu}%");
    }

    #[test]
    fn test_parse_gnu_cpu_busy() {
        let header =
            "\n%Cpu(s): 25.3 us,  5.2 sy,  0.0 ni, 60.1 id,  9.4 wa,  0.0 hi,  0.0 si,  0.0 st\n";
        let cpu = parse_top_cpu_header(header.as_bytes()).unwrap();
        assert!((cpu - 39.9).abs() < 0.5, "expected ~39.9%, got {cpu}%");
    }

    #[test]
    fn test_parse_top_cpu_no_line() {
        assert!(parse_top_cpu_header(b"no cpu here\n").is_err());
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
