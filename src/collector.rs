use std::{path::PathBuf, sync::Arc, time::Duration};

use tokio::{
    process::Command,
    time::{MissedTickBehavior, interval},
};

use crate::{
    config::Config,
    models::{CpuTimes, SystemSample},
    state::SharedState,
    system,
};

const VCGEN_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub struct Collector {
    cpu_temp_path: Option<PathBuf>,
    vcgencmd_available: bool,
    previous_cpu: Option<Vec<CpuTimes>>,
}

impl Collector {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cpu_temp_path: system::detect_cpu_temp_path(),
            vcgencmd_available: true,
            previous_cpu: None,
        }
    }

    pub async fn run(&mut self, shared: Arc<SharedState>, config: Config) {
        let mut ticker = interval(Duration::from_secs(config.interval_seconds));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            if let Ok(sample) = self.collect_sample().await {
                let mut state = shared.inner.write().await;
                state.current = sample.clone();
                state.history.push_back(sample);
                if state.history.len() > shared.history_size {
                    state.history.pop_front();
                }
            } else {
                let mut state = shared.inner.write().await;
                state.sample_errors += 1;
            }
        }
    }

    async fn collect_sample(&mut self) -> Result<SystemSample, String> {
        let cpu_snapshot = system::read_cpu_snapshot()?;
        let (cpu_usage_percent, cpu_per_core_percent) =
            system::compute_cpu_usage(&self.previous_cpu, &cpu_snapshot);
        self.previous_cpu = Some(cpu_snapshot);

        let meminfo = system::read_meminfo()?;
        let diskinfo = system::read_disk_usage("/")?;
        let uptime_seconds = system::read_uptime_seconds()?;
        let (loadavg_1, loadavg_5) = system::read_loadavg()?;

        let cpu_temp_c = self
            .cpu_temp_path
            .as_deref()
            .and_then(system::read_temp_from_path);
        let gpu_temp_c = self.read_vcgencmd_temp("measure_temp").await;
        let core_volts = self.read_vcgencmd_volts("core").await;
        let sdram_core_volts = self.read_vcgencmd_volts("sdram_c").await;
        let sdram_io_volts = self.read_vcgencmd_volts("sdram_i").await;
        let sdram_phy_volts = self.read_vcgencmd_volts("sdram_p").await;
        let arm_clock_hz = self.read_vcgencmd_clock("arm").await;
        let gpu_clock_hz = self.read_vcgencmd_clock("core").await;
        let throttled_raw = self.read_vcgencmd_throttled().await;
        let throttle_flags = system::decode_throttled(throttled_raw);

        Ok(SystemSample {
            timestamp_unix_ms: system::unix_now_ms(),
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
            sdram_c_volts: sdram_core_volts,
            sdram_i_volts: sdram_io_volts,
            sdram_p_volts: sdram_phy_volts,
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
        system::parse_temp_output(&output)
    }

    async fn read_vcgencmd_volts(&mut self, measure: &str) -> Option<f32> {
        let output = self.vcgencmd_output(["measure_volts", measure]).await?;
        system::parse_volts_output(&output)
    }

    async fn read_vcgencmd_clock(&mut self, measure: &str) -> Option<u64> {
        let output = self.vcgencmd_output(["measure_clock", measure]).await?;
        system::parse_clock_output(&output)
    }

    async fn read_vcgencmd_throttled(&mut self) -> Option<u32> {
        let output = self.vcgencmd_output(["get_throttled"]).await?;
        system::parse_throttled_output(&output)
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

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}
