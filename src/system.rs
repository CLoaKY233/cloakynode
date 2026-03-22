use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rustix::fs::statvfs;

use crate::models::{CpuTimes, DiskInfo, MemInfo, ThrottleFlags};

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

    for line in raw.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("MemTotal:") => total_kb = parts.next().and_then(|value| value.parse().ok()),
            Some("MemAvailable:") => {
                available_kb = parts.next().and_then(|value| value.parse().ok());
            }
            _ => {}
        }
    }

    let total_kb = total_kb.ok_or_else(|| "MemTotal missing".to_string())?;
    let available_kb = available_kb.ok_or_else(|| "MemAvailable missing".to_string())?;
    let used_kb = total_kb.saturating_sub(available_kb);

    Ok(MemInfo {
        total_bytes: total_kb * 1024,
        used_bytes: used_kb * 1024,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meminfo() {
        let mem = parse_meminfo(
            "MemTotal:        947952 kB\nMemFree:          73348 kB\nMemAvailable:    531140 kB\n",
        )
        .expect("meminfo should parse");

        assert_eq!(mem.total_bytes, 947_952 * 1024);
        assert_eq!(mem.used_bytes, (947_952 - 531_140) * 1024);
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
