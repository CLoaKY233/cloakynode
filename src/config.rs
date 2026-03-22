use std::{
    env,
    net::{IpAddr, Ipv4Addr},
};

pub const DEFAULT_HISTORY_SIZE: usize = 120;
pub const DEFAULT_INTERVAL_SECONDS: u64 = 5;
pub const DEFAULT_PORT: u16 = 8080;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: IpAddr,
    pub port: u16,
    pub interval_seconds: u64,
    pub history_size: usize,
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

impl Config {
    #[must_use]
    pub fn from_env_and_args() -> Self {
        let mut config = Self::default();

        if let Ok(value) = env::var("CLOAKYNODE_HOST")
            && let Ok(parsed) = value.parse()
        {
            config.host = parsed;
        }
        if let Ok(value) = env::var("CLOAKYNODE_PORT")
            && let Ok(parsed) = value.parse()
        {
            config.port = parsed;
        }
        if let Ok(value) = env::var("CLOAKYNODE_INTERVAL_SECONDS")
            && let Ok(parsed) = value.parse()
        {
            config.interval_seconds = parsed;
        }
        if let Ok(value) = env::var("CLOAKYNODE_HISTORY_SIZE")
            && let Ok(parsed) = value.parse()
        {
            config.history_size = parsed;
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
