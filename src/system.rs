use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rustix::fs::statvfs;

use crate::models::{
    CpuTimes, DiskInfo, MemInfo, NetworkInterface, ProcessCpuTimes, ProcessInfo, ThrottleFlags,
};

#[must_use]
pub fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[must_use]
pub fn read_hostname() -> Option<String> {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[must_use]
pub fn detect_cpu_temp_path() -> Option<PathBuf> {
    let thermal_root = Path::new("/sys/class/thermal");
    let mut fallback = None;

    for entry in std::fs::read_dir(thermal_root).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("thermal_zone"))
        {
            continue;
        }

        let temp_path = path.join("temp");
        if !temp_path.exists() {
            continue;
        }

        let type_path = path.join("type");
        let zone_type = std::fs::read_to_string(type_path).unwrap_or_default();
        let zone_type = zone_type.trim().to_ascii_lowercase();

        if fallback.is_none() {
            fallback = Some(temp_path.clone());
        }

        if zone_type.contains("cpu")
            || zone_type.contains("soc")
            || zone_type.contains("package")
            || zone_type.contains("thermal")
        {
            return Some(temp_path);
        }
    }

    fallback
}

#[must_use]
pub fn read_temp_from_path(path: &Path) -> Option<f32> {
    let raw = std::fs::read_to_string(path).ok()?;
    let milli_c = raw.trim().parse::<f32>().ok()?;
    Some(milli_c / 1000.0)
}

/// Read CPU counters from `/proc/stat`.
///
/// # Errors
///
/// Returns an error if `/proc/stat` cannot be read or parsed.
pub fn read_cpu_snapshot() -> Result<Vec<CpuTimes>, String> {
    let raw = std::fs::read_to_string("/proc/stat").map_err(|error| error.to_string())?;
    parse_cpu_snapshot(&raw)
}

/// Parse CPU counters from `/proc/stat` content.
///
/// # Errors
///
/// Returns an error if the content is malformed or contains no CPU counters.
pub fn parse_cpu_snapshot(raw: &str) -> Result<Vec<CpuTimes>, String> {
    let mut snapshots = Vec::new();

    for line in raw.lines() {
        if !line.starts_with("cpu") {
            break;
        }
        let mut parts = line.split_whitespace();
        let Some(label) = parts.next() else {
            continue;
        };
        if label != "cpu" && !label.starts_with("cpu") {
            continue;
        }

        let values = parts
            .take(10)
            .map(str::parse::<u64>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;

        if values.len() < 4 {
            continue;
        }

        let idle = values[3] + values.get(4).copied().unwrap_or(0);
        let total = values.iter().sum();
        snapshots.push(CpuTimes { idle, total });
    }

    if snapshots.is_empty() {
        return Err("missing cpu counters".to_string());
    }

    Ok(snapshots)
}

#[must_use]
pub fn compute_cpu_usage(
    previous: &Option<Vec<CpuTimes>>,
    current: &[CpuTimes],
) -> (f32, Vec<f32>) {
    let Some(previous) = previous else {
        return (0.0, vec![0.0; current.len().saturating_sub(1)]);
    };

    let aggregate = cpu_percent(previous.first().copied(), current.first().copied());
    let per_core = previous
        .iter()
        .skip(1)
        .zip(current.iter().skip(1))
        .map(|(prev, cur)| cpu_percent(Some(*prev), Some(*cur)))
        .collect::<Vec<_>>();

    (aggregate, per_core)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn cpu_percent(previous: Option<CpuTimes>, current: Option<CpuTimes>) -> f32 {
    let (Some(previous), Some(current)) = (previous, current) else {
        return 0.0;
    };

    let total_delta = current.total.saturating_sub(previous.total);
    if total_delta == 0 {
        return 0.0;
    }

    let idle_delta = current.idle.saturating_sub(previous.idle);
    let busy_delta = total_delta.saturating_sub(idle_delta);
    ((busy_delta as f64 / total_delta as f64) * 100.0) as f32
}

/// Read memory counters from `/proc/meminfo`.
///
/// # Errors
///
/// Returns an error if `/proc/meminfo` cannot be read or parsed.
pub fn read_meminfo() -> Result<MemInfo, String> {
    let raw = std::fs::read_to_string("/proc/meminfo").map_err(|error| error.to_string())?;
    parse_meminfo(&raw)
}

/// Parse memory counters from `/proc/meminfo` content.
///
/// # Errors
///
/// Returns an error if required fields are missing.
pub fn parse_meminfo(raw: &str) -> Result<MemInfo, String> {
    let mut total_kb: Option<u64> = None;
    let mut available_kb: Option<u64> = None;
    let mut buffers_kb: u64 = 0;
    let mut cached_kb: u64 = 0;
    let mut shmem_kb: u64 = 0;
    let mut swap_total_kb: u64 = 0;
    let mut swap_free_kb: u64 = 0;

    for line in raw.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("MemTotal:") => total_kb = parts.next().and_then(|value| value.parse().ok()),
            Some("MemAvailable:") => {
                available_kb = parts.next().and_then(|value| value.parse().ok());
            }
            Some("Buffers:") => buffers_kb = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            Some("Cached:") => cached_kb = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            Some("Shmem:") => shmem_kb = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            Some("SwapTotal:") => {
                swap_total_kb = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            }
            Some("SwapFree:") => {
                swap_free_kb = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            }
            _ => {}
        }
    }

    let total_kb = total_kb.ok_or_else(|| "MemTotal missing".to_string())?;
    let available_kb = available_kb.ok_or_else(|| "MemAvailable missing".to_string())?;
    let used_kb = total_kb.saturating_sub(available_kb);
    let swap_used_kb = swap_total_kb.saturating_sub(swap_free_kb);

    Ok(MemInfo {
        total_bytes: total_kb * 1024,
        used_bytes: used_kb * 1024,
        buffers_bytes: buffers_kb * 1024,
        cached_bytes: cached_kb * 1024,
        shared_bytes: shmem_kb * 1024,
        swap_total_bytes: swap_total_kb * 1024,
        swap_used_bytes: swap_used_kb * 1024,
    })
}

/// Read filesystem usage for a given path.
///
/// # Errors
///
/// Returns an error if filesystem stats cannot be read.
pub fn read_disk_usage(path: &str) -> Result<DiskInfo, String> {
    let stats = statvfs(path).map_err(|error| error.to_string())?;
    let block_size = stats.f_frsize;
    let total_blocks = stats.f_blocks;
    let free_blocks = stats.f_bavail;
    let total_bytes = total_blocks.saturating_mul(block_size);
    let free_bytes = free_blocks.saturating_mul(block_size);

    Ok(DiskInfo {
        total_bytes,
        used_bytes: total_bytes.saturating_sub(free_bytes),
    })
}

/// Read uptime in seconds from `/proc/uptime`.
///
/// # Errors
///
/// Returns an error if `/proc/uptime` cannot be read or parsed.
pub fn read_uptime_seconds() -> Result<f64, String> {
    let raw = std::fs::read_to_string("/proc/uptime").map_err(|error| error.to_string())?;
    raw.split_whitespace()
        .next()
        .ok_or_else(|| "uptime missing".to_string())?
        .parse::<f64>()
        .map_err(|error| error.to_string())
}

/// Read load averages from `/proc/loadavg`.
///
/// # Errors
///
/// Returns an error if `/proc/loadavg` cannot be read or parsed.
pub fn read_loadavg() -> Result<(f32, f32), String> {
    let raw = std::fs::read_to_string("/proc/loadavg").map_err(|error| error.to_string())?;
    parse_loadavg(&raw)
}

/// Parse the 1-minute and 5-minute load average values.
///
/// # Errors
///
/// Returns an error if the expected fields are missing or malformed.
pub fn parse_loadavg(raw: &str) -> Result<(f32, f32), String> {
    let mut parts = raw.split_whitespace();
    let one = parts
        .next()
        .ok_or_else(|| "loadavg 1 missing".to_string())?
        .parse::<f32>()
        .map_err(|error| error.to_string())?;
    let five = parts
        .next()
        .ok_or_else(|| "loadavg 5 missing".to_string())?
        .parse::<f32>()
        .map_err(|error| error.to_string())?;
    Ok((one, five))
}

#[must_use]
pub fn parse_temp_output(raw: &str) -> Option<f32> {
    let value = raw.trim().strip_prefix("temp=")?.strip_suffix("'C")?;
    value.parse().ok()
}

#[must_use]
pub fn parse_volts_output(raw: &str) -> Option<f32> {
    let value = raw.trim().split('=').nth(1)?.strip_suffix('V')?;
    value.parse().ok()
}

#[must_use]
pub fn parse_clock_output(raw: &str) -> Option<u64> {
    raw.trim().split('=').nth(1)?.parse().ok()
}

#[must_use]
pub fn parse_throttled_output(raw: &str) -> Option<u32> {
    let value = raw.trim().split('=').nth(1)?;
    let hex = value.strip_prefix("0x").unwrap_or(value);
    u32::from_str_radix(hex, 16).ok()
}

#[must_use]
pub fn decode_throttled(raw: Option<u32>) -> ThrottleFlags {
    let Some(raw) = raw else {
        return ThrottleFlags::default();
    };

    ThrottleFlags {
        under_voltage_now: raw & (1 << 0) != 0,
        freq_capped_now: raw & (1 << 1) != 0,
        throttled_now: raw & (1 << 2) != 0,
        soft_temp_limit_now: raw & (1 << 3) != 0,
        under_voltage_past: raw & (1 << 16) != 0,
        freq_capped_past: raw & (1 << 17) != 0,
        throttled_past: raw & (1 << 18) != 0,
        soft_temp_limit_past: raw & (1 << 19) != 0,
    }
}

/// Read network interface statistics from `/proc/net/dev`.
///
/// # Errors
///
/// Returns an error if `/proc/net/dev` cannot be read.
pub fn read_network_stats() -> Result<Vec<NetworkInterface>, String> {
    let raw = std::fs::read_to_string("/proc/net/dev").map_err(|error| error.to_string())?;
    parse_network_stats(&raw)
}

/// Parse network interface statistics from `/proc/net/dev` content.
///
/// # Errors
///
/// Returns an error if the content is malformed.
pub fn parse_network_stats(raw: &str) -> Result<Vec<NetworkInterface>, String> {
    let mut interfaces = Vec::new();

    for line in raw.lines().skip(2) {
        let Some((name, stats)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().to_string();
        if name.is_empty() || name == "lo" {
            continue;
        }

        let parts: Vec<&str> = stats.split_whitespace().collect();
        if parts.len() < 16 {
            continue;
        }

        let rx_bytes = parts[0].parse().unwrap_or(0);
        let rx_packets = parts[1].parse().unwrap_or(0);
        let tx_bytes = parts[8].parse().unwrap_or(0);
        let tx_packets = parts[9].parse().unwrap_or(0);

        interfaces.push(NetworkInterface {
            name,
            rx_bytes,
            tx_bytes,
            rx_packets,
            tx_packets,
        });
    }

    Ok(interfaces)
}

const TOP_PROCESSES: usize = 10;

/// Read top processes by CPU usage from `/proc`.
///
/// # Errors
///
/// Returns an error if `/proc` cannot be read.
#[allow(clippy::implicit_hasher)]
pub fn read_top_processes<S: std::hash::BuildHasher>(
    previous: &HashMap<u32, ProcessCpuTimes, S>,
    hz: u64,
) -> Result<(Vec<ProcessInfo>, HashMap<u32, ProcessCpuTimes>), String> {
    let proc_path = Path::new("/proc");
    let mut current_times = HashMap::new();
    let mut processes = Vec::new();

    let entries = std::fs::read_dir(proc_path).map_err(|error| error.to_string())?;

    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let Ok(pid) = name_str.parse::<u32>() else {
            continue;
        };

        let stat_path = entry.path().join("stat");
        let Ok(stat_raw) = std::fs::read_to_string(&stat_path) else {
            continue;
        };

        let Some((proc_info, cpu_times)) = parse_process_stat(&stat_raw, pid, previous, hz) else {
            continue;
        };

        current_times.insert(pid, cpu_times);
        processes.push(proc_info);
    }

    processes.sort_by(|a, b| {
        b.cpu_percent
            .partial_cmp(&a.cpu_percent)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    processes.truncate(TOP_PROCESSES);

    Ok((processes, current_times))
}

#[allow(clippy::implicit_hasher)]
fn parse_process_stat<S: std::hash::BuildHasher>(
    raw: &str,
    pid: u32,
    previous: &HashMap<u32, ProcessCpuTimes, S>,
    hz: u64,
) -> Option<(ProcessInfo, ProcessCpuTimes)> {
    let open_paren = raw.find('(')?;
    let close_paren = raw.rfind(')')?;
    let comm = raw[open_paren + 1..close_paren].to_string();

    let after_comm = &raw[close_paren + 1..];
    let parts: Vec<&str> = after_comm.split_whitespace().collect();
    if parts.len() < 22 {
        return None;
    }

    let state = parts[0].chars().next()?;
    let utime: u64 = parts[11].parse().ok()?;
    let stime: u64 = parts[12].parse().ok()?;
    let rss_pages: u64 = parts[21].parse().ok()?;
    let page_size = 4096_u64;
    let mem_bytes = rss_pages.saturating_mul(page_size);

    let total_jiffies = utime.saturating_add(stime);
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let cpu_percent = if let Some(prev) = previous.get(&pid) {
        let jiffies_delta = total_jiffies.saturating_sub(prev.total_jiffies) as f64;
        ((jiffies_delta / hz as f64) * 100.0) as f32
    } else {
        0.0
    };

    Some((
        ProcessInfo {
            pid,
            name: comm,
            cpu_percent,
            mem_bytes,
            state,
        },
        ProcessCpuTimes { pid, total_jiffies },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meminfo() {
        let mem = parse_meminfo(
            "MemTotal:        947952 kB\nMemFree:          73348 kB\nMemAvailable:    531140 kB\nBuffers:          32768 kB\nCached:          128000 kB\nShmem:            16384 kB\nSwapTotal:       524288 kB\nSwapFree:        524224 kB\n",
        )
        .expect("meminfo should parse");

        assert_eq!(mem.total_bytes, 947_952 * 1024);
        assert_eq!(mem.used_bytes, (947_952 - 531_140) * 1024);
        assert_eq!(mem.buffers_bytes, 32_768 * 1024);
        assert_eq!(mem.cached_bytes, 128_000 * 1024);
        assert_eq!(mem.shared_bytes, 16_384 * 1024);
        assert_eq!(mem.swap_total_bytes, 524_288 * 1024);
        assert_eq!(mem.swap_used_bytes, 64 * 1024);
    }

    #[test]
    fn parses_network_stats() {
        let raw = "Inter-|   Receive                                                |  Transmit\n face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed\n  eth0: 1234567   8901    0    0    0     0          0         0 9876543   4321    0    0    0     0       0          0\n    lo:       0       0    0    0    0     0          0         0        0       0    0    0    0       0          0\n";
        let interfaces = parse_network_stats(raw).expect("network stats should parse");
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].name, "eth0");
        assert_eq!(interfaces[0].rx_bytes, 1_234_567);
        assert_eq!(interfaces[0].tx_bytes, 9_876_543);
    }

    #[test]
    fn parses_loadavg() {
        let load = parse_loadavg("0.42 0.67 0.80 1/301 1234\n").expect("loadavg should parse");
        assert_eq!(load, (0.42, 0.67));
    }

    #[test]
    fn parses_cpu_snapshot() {
        let stats = parse_cpu_snapshot(
            "cpu  2255 34 2290 22625563 6290 127 456\ncpu0 1132 17 1441 11311771 3675 127 438\ncpu1 1123 17 849 11313792 2614 0 18\nintr 123\n",
        )
        .expect("cpu snapshot should parse");

        assert_eq!(stats.len(), 3);
        assert_eq!(stats[0].idle, 22_625_563 + 6290);
    }

    #[test]
    fn decodes_throttled_flags() {
        let flags = decode_throttled(Some(0x50005));
        assert!(flags.under_voltage_now);
        assert!(flags.throttled_now);
        assert!(flags.under_voltage_past);
        assert!(flags.throttled_past);
        assert!(!flags.freq_capped_now);
    }

    #[test]
    fn parses_vcgencmd_outputs() {
        assert_eq!(parse_temp_output("temp=48.7'C\n"), Some(48.7));
        assert_eq!(parse_volts_output("volt=1.2000V\n"), Some(1.2));
        assert_eq!(
            parse_clock_output("frequency(48)=1500000000\n"),
            Some(1_500_000_000)
        );
        assert_eq!(parse_throttled_output("throttled=0x50005\n"), Some(0x50005));
    }
}
