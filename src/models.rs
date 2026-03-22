use serde::{Deserialize, Serialize};

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemSample {
    pub timestamp_unix_ms: u64,
    pub cpu_usage_percent: f32,
    pub cpu_per_core_percent: Vec<f32>,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub uptime_seconds: f64,
    pub loadavg_1: f32,
    pub loadavg_5: f32,
    pub cpu_temp_c: Option<f32>,
    pub gpu_temp_c: Option<f32>,
    pub core_volts: Option<f32>,
    pub sdram_c_volts: Option<f32>,
    pub sdram_i_volts: Option<f32>,
    pub sdram_p_volts: Option<f32>,
    pub arm_clock_hz: Option<u64>,
    pub gpu_clock_hz: Option<u64>,
    pub throttled_raw: Option<u32>,
    pub under_voltage_now: bool,
    pub freq_capped_now: bool,
    pub throttled_now: bool,
    pub soft_temp_limit_now: bool,
    pub under_voltage_past: bool,
    pub freq_capped_past: bool,
    pub throttled_past: bool,
    pub soft_temp_limit_past: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub process_uptime_seconds: u64,
    pub last_sample_timestamp_unix_ms: u64,
    pub sample_errors: u64,
    pub request_count: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CpuTimes {
    pub idle: u64,
    pub total: u64,
}

#[derive(Debug)]
pub struct MemInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
}

#[derive(Debug)]
pub struct DiskInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ThrottleFlags {
    pub under_voltage_now: bool,
    pub freq_capped_now: bool,
    pub throttled_now: bool,
    pub soft_temp_limit_now: bool,
    pub under_voltage_past: bool,
    pub freq_capped_past: bool,
    pub throttled_past: bool,
    pub soft_temp_limit_past: bool,
}
