use std::{
    collections::VecDeque,
    env,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::{Query, State},
    response::{Html, IntoResponse},
    routing::get,
};
use rustix::fs::statvfs;
use serde::{Deserialize, Serialize};
use tokio::{
    net::TcpListener,
    process::Command,
    sync::RwLock,
    time::{MissedTickBehavior, interval},
};

const DEFAULT_HISTORY_SIZE: usize = 120;
const DEFAULT_INTERVAL_SECONDS: u64 = 5;
const DEFAULT_PORT: u16 = 8080;
const VCGEN_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Clone)]
struct Config {
    host: IpAddr,
    port: u16,
    interval_seconds: u64,
    history_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: DEFAULT_PORT,
            interval_seconds: DEFAULT_INTERVAL_SECONDS,
            history_size: DEFAULT_HISTORY_SIZE,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SystemSample {
    timestamp_unix_ms: u64,
    cpu_usage_percent: f32,
    cpu_per_core_percent: Vec<f32>,
    memory_used_bytes: u64,
    memory_total_bytes: u64,
    disk_used_bytes: u64,
    disk_total_bytes: u64,
    uptime_seconds: f64,
    loadavg_1: f32,
    loadavg_5: f32,
    cpu_temp_c: Option<f32>,
    gpu_temp_c: Option<f32>,
    core_volts: Option<f32>,
    sdram_c_volts: Option<f32>,
    sdram_i_volts: Option<f32>,
    sdram_p_volts: Option<f32>,
    arm_clock_hz: Option<u64>,
    gpu_clock_hz: Option<u64>,
    throttled_raw: Option<u32>,
    under_voltage_now: bool,
    freq_capped_now: bool,
    throttled_now: bool,
    soft_temp_limit_now: bool,
    under_voltage_past: bool,
    freq_capped_past: bool,
    throttled_past: bool,
    soft_temp_limit_past: bool,
}

#[derive(Debug, Clone, Serialize)]
struct HealthResponse {
    status: &'static str,
    process_uptime_seconds: u64,
    last_sample_timestamp_unix_ms: u64,
    sample_errors: u64,
    request_count: u64,
}

#[derive(Debug, Default)]
struct RuntimeState {
    current: SystemSample,
    history: VecDeque<SystemSample>,
    sample_errors: u64,
}

#[derive(Debug)]
struct SharedState {
    hostname: String,
    started_at: Instant,
    history_size: usize,
    request_count: AtomicU64,
    inner: RwLock<RuntimeState>,
}

#[derive(Debug, Clone)]
struct AppState {
    shared: Arc<SharedState>,
}

#[derive(Debug, Clone, Copy, Default)]
struct CpuTimes {
    idle: u64,
    total: u64,
}

#[derive(Debug)]
struct Collector {
    cpu_temp_path: Option<PathBuf>,
    vcgencmd_available: bool,
    previous_cpu: Option<Vec<CpuTimes>>,
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env_and_args();
    let hostname = read_hostname().unwrap_or_else(|| "raspberrypi".to_string());

    let shared = Arc::new(SharedState {
        hostname,
        started_at: Instant::now(),
        history_size: config.history_size,
        request_count: AtomicU64::new(0),
        inner: RwLock::new(RuntimeState {
            history: VecDeque::with_capacity(config.history_size),
            ..RuntimeState::default()
        }),
    });

    let collector_state = Arc::clone(&shared);
    let collector_config = config.clone();
    let collector_task = tokio::spawn(async move {
        let mut collector = Collector::new();
        collector.run(collector_state, collector_config).await;
    });

    let app_state = AppState { shared };
    let app = Router::new()
        .route("/", get(index))
        .route("/api/current", get(current_sample))
        .route("/api/history", get(history))
        .route("/api/health", get(health))
        .with_state(app_state);

    let addr = SocketAddr::new(config.host, config.port);
    let listener = TcpListener::bind(addr).await?;

    println!("listening on http://{addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    collector_task.abort();
    Ok(())
}

impl Config {
    fn from_env_and_args() -> Self {
        let mut config = Self::default();

        if let Ok(value) = env::var("CLOAKYNODE_HOST") {
            if let Ok(parsed) = value.parse() {
                config.host = parsed;
            }
        }
        if let Ok(value) = env::var("CLOAKYNODE_PORT") {
            if let Ok(parsed) = value.parse() {
                config.port = parsed;
            }
        }
        if let Ok(value) = env::var("CLOAKYNODE_INTERVAL_SECONDS") {
            if let Ok(parsed) = value.parse() {
                config.interval_seconds = parsed;
            }
        }
        if let Ok(value) = env::var("CLOAKYNODE_HISTORY_SIZE") {
            if let Ok(parsed) = value.parse() {
                config.history_size = parsed;
            }
        }

        let mut args = env::args().skip(1);
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--host" => {
                    if let Some(value) = args.next().and_then(|v| v.parse().ok()) {
                        config.host = value;
                    }
                }
                "--port" => {
                    if let Some(value) = args.next().and_then(|v| v.parse().ok()) {
                        config.port = value;
                    }
                }
                "--interval-seconds" => {
                    if let Some(value) = args.next().and_then(|v| v.parse().ok()) {
                        config.interval_seconds = value;
                    }
                }
                "--history-size" => {
                    if let Some(value) = args.next().and_then(|v| v.parse().ok()) {
                        config.history_size = value;
                    }
                }
                _ => {}
            }
        }

        config.interval_seconds = config.interval_seconds.max(1);
        config.history_size = config.history_size.clamp(12, 1_024);
        config
    }
}

impl Collector {
    fn new() -> Self {
        Self {
            cpu_temp_path: detect_cpu_temp_path(),
            vcgencmd_available: true,
            previous_cpu: None,
        }
    }

    async fn run(&mut self, shared: Arc<SharedState>, config: Config) {
        let mut ticker = interval(Duration::from_secs(config.interval_seconds));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            match self.collect_sample().await {
                Ok(sample) => {
                    let mut state = shared.inner.write().await;
                    state.current = sample.clone();
                    state.history.push_back(sample);
                    if state.history.len() > shared.history_size {
                        state.history.pop_front();
                    }
                }
                Err(_) => {
                    let mut state = shared.inner.write().await;
                    state.sample_errors += 1;
                }
            }
        }
    }

    async fn collect_sample(&mut self) -> Result<SystemSample, String> {
        let cpu_snapshot = read_cpu_snapshot()?;
        let (cpu_usage_percent, cpu_per_core_percent) =
            compute_cpu_usage(&self.previous_cpu, &cpu_snapshot);
        self.previous_cpu = Some(cpu_snapshot);

        let meminfo = read_meminfo()?;
        let diskinfo = read_disk_usage("/")?;
        let uptime_seconds = read_uptime_seconds()?;
        let (loadavg_1, loadavg_5) = read_loadavg()?;

        let cpu_temp_c = self.cpu_temp_path.as_deref().and_then(read_temp_from_path);
        let gpu_temp_c = self.read_vcgencmd_temp("measure_temp").await;
        let core_volts = self.read_vcgencmd_volts("core").await;
        let sdram_c_volts = self.read_vcgencmd_volts("sdram_c").await;
        let sdram_i_volts = self.read_vcgencmd_volts("sdram_i").await;
        let sdram_p_volts = self.read_vcgencmd_volts("sdram_p").await;
        let arm_clock_hz = self.read_vcgencmd_clock("arm").await;
        let gpu_clock_hz = self.read_vcgencmd_clock("core").await;
        let throttled_raw = self.read_vcgencmd_throttled().await;
        let throttle_flags = decode_throttled(throttled_raw);

        Ok(SystemSample {
            timestamp_unix_ms: unix_now_ms(),
            cpu_usage_percent,
            cpu_per_core_percent,
            memory_used_bytes: meminfo.used_bytes,
            memory_total_bytes: meminfo.total_bytes,
            disk_used_bytes: diskinfo.used_bytes,
            disk_total_bytes: diskinfo.total_bytes,
            uptime_seconds,
            loadavg_1,
            loadavg_5,
            cpu_temp_c,
            gpu_temp_c,
            core_volts,
            sdram_c_volts,
            sdram_i_volts,
            sdram_p_volts,
            arm_clock_hz,
            gpu_clock_hz,
            throttled_raw,
            under_voltage_now: throttle_flags.under_voltage_now,
            freq_capped_now: throttle_flags.freq_capped_now,
            throttled_now: throttle_flags.throttled_now,
            soft_temp_limit_now: throttle_flags.soft_temp_limit_now,
            under_voltage_past: throttle_flags.under_voltage_past,
            freq_capped_past: throttle_flags.freq_capped_past,
            throttled_past: throttle_flags.throttled_past,
            soft_temp_limit_past: throttle_flags.soft_temp_limit_past,
        })
    }

    async fn read_vcgencmd_temp(&mut self, command: &str) -> Option<f32> {
        let output = self.vcgencmd_output([command]).await?;
        parse_temp_output(&output)
    }

    async fn read_vcgencmd_volts(&mut self, measure: &str) -> Option<f32> {
        let output = self.vcgencmd_output(["measure_volts", measure]).await?;
        parse_volts_output(&output)
    }

    async fn read_vcgencmd_clock(&mut self, measure: &str) -> Option<u64> {
        let output = self.vcgencmd_output(["measure_clock", measure]).await?;
        parse_clock_output(&output)
    }

    async fn read_vcgencmd_throttled(&mut self) -> Option<u32> {
        let output = self.vcgencmd_output(["get_throttled"]).await?;
        parse_throttled_output(&output)
    }

    async fn vcgencmd_output<const N: usize>(&mut self, args: [&str; N]) -> Option<String> {
        if !self.vcgencmd_available {
            return None;
        }

        let mut command = Command::new("vcgencmd");
        command.args(args);
        let result = tokio::time::timeout(VCGEN_TIMEOUT, command.output()).await;
        let output = match result {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                if error.kind() == std::io::ErrorKind::NotFound {
                    self.vcgencmd_available = false;
                }
                return None;
            }
            Err(_) => return None,
        };

        if !output.status.success() {
            return None;
        }

        String::from_utf8(output.stdout).ok()
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn index(State(state): State<AppState>) -> impl IntoResponse {
    bump_request_count(&state.shared);
    Html(build_index_html(&state.shared.hostname))
}

async fn current_sample(State(state): State<AppState>) -> impl IntoResponse {
    bump_request_count(&state.shared);
    let current = {
        let inner = state.shared.inner.read().await;
        inner.current.clone()
    };
    Json(current)
}

async fn history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> impl IntoResponse {
    bump_request_count(&state.shared);
    let limit = query.limit.unwrap_or(state.shared.history_size);
    let clamped = limit.clamp(1, state.shared.history_size);

    let history = {
        let inner = state.shared.inner.read().await;
        let len = inner.history.len();
        let start = len.saturating_sub(clamped);
        inner
            .history
            .iter()
            .skip(start)
            .cloned()
            .collect::<Vec<_>>()
    };

    Json(history)
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    bump_request_count(&state.shared);
    let response = {
        let inner = state.shared.inner.read().await;
        HealthResponse {
            status: "ok",
            process_uptime_seconds: state.shared.started_at.elapsed().as_secs(),
            last_sample_timestamp_unix_ms: inner.current.timestamp_unix_ms,
            sample_errors: inner.sample_errors,
            request_count: state.shared.request_count.load(Ordering::Relaxed),
        }
    };
    Json(response)
}

fn bump_request_count(shared: &SharedState) {
    shared.request_count.fetch_add(1, Ordering::Relaxed);
}

fn build_index_html(hostname: &str) -> String {
    INDEX_HTML.replace("__HOSTNAME__", hostname)
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn read_hostname() -> Option<String> {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn detect_cpu_temp_path() -> Option<PathBuf> {
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

fn read_temp_from_path(path: &Path) -> Option<f32> {
    let raw = std::fs::read_to_string(path).ok()?;
    let milli_c = raw.trim().parse::<f32>().ok()?;
    Some(milli_c / 1000.0)
}

fn read_cpu_snapshot() -> Result<Vec<CpuTimes>, String> {
    let raw = std::fs::read_to_string("/proc/stat").map_err(|error| error.to_string())?;
    parse_cpu_snapshot(&raw)
}

fn parse_cpu_snapshot(raw: &str) -> Result<Vec<CpuTimes>, String> {
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

fn compute_cpu_usage(previous: &Option<Vec<CpuTimes>>, current: &[CpuTimes]) -> (f32, Vec<f32>) {
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

#[derive(Debug)]
struct MemInfo {
    total_bytes: u64,
    used_bytes: u64,
}

fn read_meminfo() -> Result<MemInfo, String> {
    let raw = std::fs::read_to_string("/proc/meminfo").map_err(|error| error.to_string())?;
    parse_meminfo(&raw)
}

fn parse_meminfo(raw: &str) -> Result<MemInfo, String> {
    let mut total_kb: Option<u64> = None;
    let mut available_kb: Option<u64> = None;

    for line in raw.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("MemTotal:") => total_kb = parts.next().and_then(|value| value.parse().ok()),
            Some("MemAvailable:") => {
                available_kb = parts.next().and_then(|value| value.parse().ok())
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

#[derive(Debug)]
struct DiskInfo {
    total_bytes: u64,
    used_bytes: u64,
}

fn read_disk_usage(path: &str) -> Result<DiskInfo, String> {
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

fn read_uptime_seconds() -> Result<f64, String> {
    let raw = std::fs::read_to_string("/proc/uptime").map_err(|error| error.to_string())?;
    raw.split_whitespace()
        .next()
        .ok_or_else(|| "uptime missing".to_string())?
        .parse::<f64>()
        .map_err(|error| error.to_string())
}

fn read_loadavg() -> Result<(f32, f32), String> {
    let raw = std::fs::read_to_string("/proc/loadavg").map_err(|error| error.to_string())?;
    parse_loadavg(&raw)
}

fn parse_loadavg(raw: &str) -> Result<(f32, f32), String> {
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

#[derive(Debug, Clone, Copy, Default)]
struct ThrottleFlags {
    under_voltage_now: bool,
    freq_capped_now: bool,
    throttled_now: bool,
    soft_temp_limit_now: bool,
    under_voltage_past: bool,
    freq_capped_past: bool,
    throttled_past: bool,
    soft_temp_limit_past: bool,
}

fn parse_temp_output(raw: &str) -> Option<f32> {
    let value = raw.trim().strip_prefix("temp=")?.strip_suffix("'C")?;
    value.parse().ok()
}

fn parse_volts_output(raw: &str) -> Option<f32> {
    let value = raw.trim().split('=').nth(1)?.strip_suffix('V')?;
    value.parse().ok()
}

fn parse_clock_output(raw: &str) -> Option<u64> {
    raw.trim().split('=').nth(1)?.parse().ok()
}

fn parse_throttled_output(raw: &str) -> Option<u32> {
    let value = raw.trim().split('=').nth(1)?;
    let hex = value.strip_prefix("0x").unwrap_or(value);
    u32::from_str_radix(hex, 16).ok()
}

fn decode_throttled(raw: Option<u32>) -> ThrottleFlags {
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

const INDEX_HTML: &str = r###"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Raspberry Pi Monitor</title>
  <style>
    :root {
      --bg: #09111d;
      --panel: rgba(10, 18, 31, 0.88);
      --panel-2: rgba(16, 28, 44, 0.9);
      --line: rgba(122, 162, 255, 0.22);
      --text: #edf4ff;
      --muted: #8ea7c4;
      --good: #3bd58f;
      --warn: #ffb54c;
      --bad: #ff6b6b;
      --accent: #63b3ff;
      --accent-2: #59f0c2;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
      color: var(--text);
      background:
        radial-gradient(circle at top left, rgba(99, 179, 255, 0.16), transparent 28%),
        radial-gradient(circle at right, rgba(89, 240, 194, 0.12), transparent 22%),
        linear-gradient(180deg, #0b1422 0%, #07101a 100%);
      min-height: 100vh;
    }
    .shell {
      max-width: 1160px;
      margin: 0 auto;
      padding: 24px;
    }
    .topbar {
      display: grid;
      gap: 16px;
      grid-template-columns: 1.8fr 1fr;
      margin-bottom: 20px;
    }
    .hero, .status {
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 22px;
      box-shadow: 0 14px 44px rgba(0, 0, 0, 0.28);
      backdrop-filter: blur(14px);
    }
    .hero {
      padding: 22px;
    }
    .eyebrow {
      color: var(--accent-2);
      font-size: 12px;
      letter-spacing: 0.16em;
      text-transform: uppercase;
    }
    h1 {
      margin: 8px 0 6px;
      font-size: clamp(28px, 5vw, 42px);
      line-height: 1;
    }
    .subtitle, .meta, .status-text {
      color: var(--muted);
    }
    .meta {
      display: flex;
      gap: 16px;
      flex-wrap: wrap;
      margin-top: 14px;
      font-size: 14px;
    }
    .status {
      padding: 18px;
      display: grid;
      gap: 14px;
      align-content: start;
    }
    .pill-row, .warn-row {
      display: flex;
      flex-wrap: wrap;
      gap: 10px;
    }
    .pill, .warn-pill {
      border-radius: 999px;
      padding: 8px 12px;
      font-size: 13px;
      border: 1px solid transparent;
    }
    .pill {
      background: rgba(255, 255, 255, 0.06);
      color: var(--muted);
    }
    .pill.live {
      color: #dffef2;
      background: rgba(59, 213, 143, 0.12);
      border-color: rgba(59, 213, 143, 0.35);
    }
    .pill.offline {
      color: #ffe2e2;
      background: rgba(255, 107, 107, 0.12);
      border-color: rgba(255, 107, 107, 0.35);
    }
    .warn-pill.good {
      background: rgba(59, 213, 143, 0.12);
      border-color: rgba(59, 213, 143, 0.28);
    }
    .warn-pill.warn {
      background: rgba(255, 181, 76, 0.12);
      border-color: rgba(255, 181, 76, 0.28);
    }
    .warn-pill.bad {
      background: rgba(255, 107, 107, 0.12);
      border-color: rgba(255, 107, 107, 0.28);
    }
    .grid {
      display: grid;
      gap: 16px;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      margin-bottom: 16px;
    }
    .card, .charts {
      background: var(--panel-2);
      border: 1px solid var(--line);
      border-radius: 20px;
      box-shadow: 0 12px 34px rgba(0, 0, 0, 0.22);
    }
    .card {
      padding: 18px;
      min-height: 148px;
    }
    .label {
      color: var(--muted);
      font-size: 13px;
      text-transform: uppercase;
      letter-spacing: 0.08em;
    }
    .value {
      margin-top: 14px;
      font-size: 34px;
      font-weight: 700;
      line-height: 1;
    }
    .detail {
      margin-top: 12px;
      color: var(--muted);
      font-size: 14px;
      min-height: 20px;
    }
    .charts {
      padding: 18px;
    }
    .chart-grid {
      display: grid;
      gap: 14px;
      grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
    }
    .chart-panel {
      padding: 14px;
      border-radius: 16px;
      background: rgba(255, 255, 255, 0.03);
      border: 1px solid rgba(255, 255, 255, 0.04);
    }
    canvas {
      width: 100%;
      height: 100px;
      display: block;
      margin-top: 10px;
    }
    .chart-value {
      color: var(--text);
      font-size: 22px;
      font-weight: 700;
    }
    @media (max-width: 860px) {
      .topbar {
        grid-template-columns: 1fr;
      }
      .shell {
        padding: 16px;
      }
    }
  </style>
</head>
<body>
  <div class="shell">
    <section class="topbar">
      <div class="hero">
        <div class="eyebrow">Raspberry Pi Monitor</div>
        <h1>__HOSTNAME__</h1>
        <div class="subtitle">Low-overhead local dashboard for Pi thermals, power and system load.</div>
        <div class="meta">
          <span id="uptimeLabel">Uptime: --</span>
          <span id="loadLabel">Load: -- / --</span>
          <span id="clockLabel">ARM: -- GHz | GPU: -- MHz</span>
        </div>
      </div>
      <aside class="status">
        <div class="pill-row">
          <span id="connectionPill" class="pill">Connecting</span>
          <span class="pill">Polling: 5s</span>
          <span class="pill">History: 10 min</span>
        </div>
        <div class="status-text" id="statusText">Waiting for the first sample.</div>
        <div class="warn-row" id="warnRow"></div>
      </aside>
    </section>

    <section class="grid">
      <article class="card">
        <div class="label">CPU Usage</div>
        <div class="value" id="cpuValue">--</div>
        <div class="detail" id="cpuDetail">Per-core activity unavailable</div>
      </article>
      <article class="card">
        <div class="label">Memory</div>
        <div class="value" id="memoryValue">--</div>
        <div class="detail" id="memoryDetail">Used vs total</div>
      </article>
      <article class="card">
        <div class="label">Disk /</div>
        <div class="value" id="diskValue">--</div>
        <div class="detail" id="diskDetail">Filesystem usage</div>
      </article>
      <article class="card">
        <div class="label">CPU Temp</div>
        <div class="value" id="tempValue">--</div>
        <div class="detail" id="tempDetail">Thermal headroom</div>
      </article>
      <article class="card">
        <div class="label">Core Voltage</div>
        <div class="value" id="voltValue">--</div>
        <div class="detail" id="voltDetail">SDRAM rails hidden until detected</div>
      </article>
      <article class="card">
        <div class="label">Throttle State</div>
        <div class="value" id="throttleValue">--</div>
        <div class="detail" id="throttleDetail">Pi firmware flags</div>
      </article>
    </section>

    <section class="charts">
      <div class="chart-grid">
        <div class="chart-panel">
          <div class="label">CPU</div>
          <div class="chart-value" id="cpuChartValue">--</div>
          <canvas id="cpuChart" width="280" height="100"></canvas>
        </div>
        <div class="chart-panel">
          <div class="label">Memory</div>
          <div class="chart-value" id="memChartValue">--</div>
          <canvas id="memChart" width="280" height="100"></canvas>
        </div>
        <div class="chart-panel">
          <div class="label">Temperature</div>
          <div class="chart-value" id="tempChartValue">--</div>
          <canvas id="tempChart" width="280" height="100"></canvas>
        </div>
        <div class="chart-panel">
          <div class="label">Core Voltage</div>
          <div class="chart-value" id="voltChartValue">--</div>
          <canvas id="voltChart" width="280" height="100"></canvas>
        </div>
      </div>
    </section>
  </div>

  <script>
    const charts = {
      cpu: document.getElementById("cpuChart"),
      mem: document.getElementById("memChart"),
      temp: document.getElementById("tempChart"),
      volt: document.getElementById("voltChart"),
    };

    function pct(used, total) {
      if (!total) return 0;
      return (used / total) * 100;
    }

    function fmtBytes(bytes) {
      if (!bytes) return "0 B";
      const units = ["B", "KB", "MB", "GB", "TB"];
      let value = bytes;
      let idx = 0;
      while (value >= 1024 && idx < units.length - 1) {
        value /= 1024;
        idx += 1;
      }
      return `${value.toFixed(value >= 10 || idx === 0 ? 0 : 1)} ${units[idx]}`;
    }

    function fmtDuration(seconds) {
      const s = Math.floor(seconds || 0);
      const days = Math.floor(s / 86400);
      const hours = Math.floor((s % 86400) / 3600);
      const mins = Math.floor((s % 3600) / 60);
      return `${days}d ${hours}h ${mins}m`;
    }

    function setConnection(ok) {
      const el = document.getElementById("connectionPill");
      el.textContent = ok ? "Connected" : "Disconnected";
      el.className = ok ? "pill live" : "pill offline";
    }

    function statusTone(value, warnAt, badAt) {
      if (value >= badAt) return "var(--bad)";
      if (value >= warnAt) return "var(--warn)";
      return "var(--good)";
    }

    function drawSpark(canvas, values, maxHint, color) {
      const ctx = canvas.getContext("2d");
      const { width, height } = canvas;
      ctx.clearRect(0, 0, width, height);

      ctx.strokeStyle = "rgba(255,255,255,0.08)";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(0, height - 1);
      ctx.lineTo(width, height - 1);
      ctx.stroke();

      const filtered = values.filter((value) => Number.isFinite(value));
      if (filtered.length < 2) return;

      const max = Math.max(maxHint, ...filtered);
      const min = Math.min(...filtered);
      const span = Math.max(max - min, maxHint > 0 ? maxHint * 0.1 : 1);

      ctx.strokeStyle = color;
      ctx.lineWidth = 2.2;
      ctx.beginPath();
      filtered.forEach((value, index) => {
        const x = (index / (filtered.length - 1)) * width;
        const normalized = (value - min) / span;
        const y = height - normalized * (height - 8) - 4;
        if (index === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      });
      ctx.stroke();
    }

    function renderWarnings(sample) {
      const row = document.getElementById("warnRow");
      row.innerHTML = "";
      const warnings = [];
      if (sample.under_voltage_now) warnings.push(["Under-voltage", "bad"]);
      if (sample.freq_capped_now) warnings.push(["Frequency capped", "warn"]);
      if (sample.throttled_now) warnings.push(["Throttled", "bad"]);
      if (sample.soft_temp_limit_now) warnings.push(["Soft temp limit", "warn"]);
      if (!warnings.length) warnings.push(["No active Pi firmware warnings", "good"]);
      warnings.forEach(([text, tone]) => {
        const pill = document.createElement("span");
        pill.className = `warn-pill ${tone}`;
        pill.textContent = text;
        row.appendChild(pill);
      });
    }

    function renderCurrent(sample) {
      const memPercent = pct(sample.memory_used_bytes, sample.memory_total_bytes);
      const diskPercent = pct(sample.disk_used_bytes, sample.disk_total_bytes);
      const cpuColor = statusTone(sample.cpu_usage_percent, 70, 90);
      const memColor = statusTone(memPercent, 70, 90);
      const tempColor = statusTone(sample.cpu_temp_c ?? 0, 60, 80);

      document.getElementById("cpuValue").textContent = `${sample.cpu_usage_percent.toFixed(1)}%`;
      document.getElementById("cpuValue").style.color = cpuColor;
      document.getElementById("cpuDetail").textContent = sample.cpu_per_core_percent.length
        ? sample.cpu_per_core_percent.map((value, index) => `c${index}: ${value.toFixed(0)}%`).join(" | ")
        : "Per-core counters pending";

      document.getElementById("memoryValue").textContent = `${memPercent.toFixed(1)}%`;
      document.getElementById("memoryValue").style.color = memColor;
      document.getElementById("memoryDetail").textContent =
        `${fmtBytes(sample.memory_used_bytes)} / ${fmtBytes(sample.memory_total_bytes)}`;

      document.getElementById("diskValue").textContent = `${diskPercent.toFixed(1)}%`;
      document.getElementById("diskDetail").textContent =
        `${fmtBytes(sample.disk_used_bytes)} / ${fmtBytes(sample.disk_total_bytes)}`;

      document.getElementById("tempValue").textContent = sample.cpu_temp_c == null ? "--" : `${sample.cpu_temp_c.toFixed(1)}°C`;
      document.getElementById("tempValue").style.color = tempColor;
      document.getElementById("tempDetail").textContent = sample.gpu_temp_c == null
        ? "GPU temp unavailable"
        : `GPU ${sample.gpu_temp_c.toFixed(1)}°C`;

      document.getElementById("voltValue").textContent = sample.core_volts == null ? "--" : `${sample.core_volts.toFixed(2)}V`;
      document.getElementById("voltDetail").textContent =
        `SDRAM C/I/P: ${sample.sdram_c_volts?.toFixed?.(2) ?? "--"} / ${sample.sdram_i_volts?.toFixed?.(2) ?? "--"} / ${sample.sdram_p_volts?.toFixed?.(2) ?? "--"} V`;

      document.getElementById("throttleValue").textContent = sample.throttled_now ? "Active" : "Clear";
      document.getElementById("throttleValue").style.color = sample.throttled_now || sample.under_voltage_now
        ? "var(--bad)"
        : "var(--good)";
      document.getElementById("throttleDetail").textContent =
        sample.throttled_raw == null ? "vcgencmd unavailable" : `Raw flags 0x${sample.throttled_raw.toString(16)}`;

      document.getElementById("uptimeLabel").textContent = `Uptime: ${fmtDuration(sample.uptime_seconds)}`;
      document.getElementById("loadLabel").textContent = `Load: ${sample.loadavg_1.toFixed(2)} / ${sample.loadavg_5.toFixed(2)}`;

      const armGHz = sample.arm_clock_hz == null ? "--" : (sample.arm_clock_hz / 1e9).toFixed(2);
      const gpuMHz = sample.gpu_clock_hz == null ? "--" : (sample.gpu_clock_hz / 1e6).toFixed(0);
      document.getElementById("clockLabel").textContent = `ARM: ${armGHz} GHz | GPU: ${gpuMHz} MHz`;
      document.getElementById("statusText").textContent = new Date(sample.timestamp_unix_ms).toLocaleTimeString();

      renderWarnings(sample);
    }

    function renderCharts(history) {
      const cpu = history.map((item) => item.cpu_usage_percent);
      const mem = history.map((item) => pct(item.memory_used_bytes, item.memory_total_bytes));
      const temp = history.map((item) => item.cpu_temp_c);
      const volt = history.map((item) => item.core_volts);

      drawSpark(charts.cpu, cpu, 100, "#63b3ff");
      drawSpark(charts.mem, mem, 100, "#59f0c2");
      drawSpark(charts.temp, temp, 90, "#ffb54c");
      drawSpark(charts.volt, volt, 2, "#ff8d73");

      const latest = history[history.length - 1];
      if (!latest) return;

      document.getElementById("cpuChartValue").textContent = `${latest.cpu_usage_percent.toFixed(1)}%`;
      document.getElementById("memChartValue").textContent = `${pct(latest.memory_used_bytes, latest.memory_total_bytes).toFixed(1)}%`;
      document.getElementById("tempChartValue").textContent = latest.cpu_temp_c == null ? "--" : `${latest.cpu_temp_c.toFixed(1)}°C`;
      document.getElementById("voltChartValue").textContent = latest.core_volts == null ? "--" : `${latest.core_volts.toFixed(2)}V`;
    }

    async function refresh() {
      try {
        const [currentResponse, historyResponse] = await Promise.all([
          fetch("/api/current", { cache: "no-store" }),
          fetch("/api/history?limit=120", { cache: "no-store" }),
        ]);
        if (!currentResponse.ok || !historyResponse.ok) throw new Error("request failed");

        const current = await currentResponse.json();
        const history = await historyResponse.json();
        setConnection(true);
        renderCurrent(current);
        renderCharts(history);
      } catch (_) {
        setConnection(false);
        document.getElementById("statusText").textContent = "Unable to reach API.";
      }
    }

    refresh();
    setInterval(refresh, 5000);
  </script>
</body>
</html>
"###;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::util::ServiceExt;

    #[test]
    fn parses_meminfo() {
        let mem = parse_meminfo(
            "MemTotal:        947952 kB\nMemFree:          73348 kB\nMemAvailable:    531140 kB\n",
        )
        .expect("meminfo should parse");

        assert_eq!(mem.total_bytes, 947952 * 1024);
        assert_eq!(mem.used_bytes, (947952 - 531140) * 1024);
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
        assert_eq!(stats[0].idle, 22625563 + 6290);
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

    #[tokio::test]
    async fn history_endpoint_clamps_limit() {
        let shared = Arc::new(SharedState {
            hostname: "pi".to_string(),
            started_at: Instant::now(),
            history_size: 3,
            request_count: AtomicU64::new(0),
            inner: RwLock::new(RuntimeState {
                current: SystemSample {
                    timestamp_unix_ms: 3,
                    ..SystemSample::default()
                },
                history: VecDeque::from(vec![
                    SystemSample {
                        timestamp_unix_ms: 1,
                        ..SystemSample::default()
                    },
                    SystemSample {
                        timestamp_unix_ms: 2,
                        ..SystemSample::default()
                    },
                    SystemSample {
                        timestamp_unix_ms: 3,
                        ..SystemSample::default()
                    },
                ]),
                sample_errors: 0,
            }),
        });

        let app = Router::new()
            .route("/api/history", get(history))
            .with_state(AppState { shared });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/history?limit=99")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should be produced");

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let samples: Vec<SystemSample> =
            serde_json::from_slice(&body).expect("history body should deserialize");

        assert_eq!(samples.len(), 3);
        assert_eq!(samples[0].timestamp_unix_ms, 1);
    }
}
