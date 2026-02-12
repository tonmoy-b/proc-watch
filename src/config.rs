use clap::Parser;
use std::time::Duration;

#[derive(Parser, Debug, Clone)]
#[command(name = "infra_health_agent", version, about)]
pub struct Config {
    /// Unique identifier for this agent instance.
    /// if none provided, default to hostname.
    #[arg(long, env = "INFRA_HEALTH_AGENT_ID")]
    pub agent_id: Option<String>,

    /// Telemetry collection interval in milliseconds.
    #[arg(long, env = "INFRA_HEALTH_COLLECT_INTERVAL_MS", default_value_t = 5000)]
    pub collect_interval_ms: u64,

    /// Reporting channel buffer size (bounded to enforce backpressure).
    #[arg(long, env = "INFRA_HEALTH_CHANNEL_BUFFER", default_value_t = 256)]
    pub channel_buffer_size: usize,

    /// Comma-separated list of PIDs to monitor.
    #[arg(long, env = "INFRA_HEALTH_MONITORED_PIDS", value_delimiter = ',')]
    pub monitored_pids: Vec<u32>,

    /// Enable JSON structured logging.
    #[arg(long, env = "INFRA_HEALTH_JSON_LOGS", default_value_t = false)]
    pub json_logs: bool,

    /// Maximum retries for failed report transmissions.
    #[arg(long, env = "INFRA_HEALTH_MAX_RETRIES", default_value_t = 3)]
    pub max_retries: u32,

    /// Retry backoff base in milliseconds.
    #[arg(long, env = "INFRA_HEALTH_RETRY_BACKOFF_MS", default_value_t = 500)]
    pub retry_backoff_ms: u64,
}

impl Config {
    /// get agent ID, upon failure fallback to hostname.
    pub fn resolved_agent_id(&self) -> String {
        self.agent_id
            .clone()
            .unwrap_or_else(|| {
                hostname::get()
                    .map(|h| h.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| "unknown-agent".to_string())
            })
    }

    pub fn collect_interval(&self) -> Duration {
        Duration::from_millis(self.collect_interval_ms)
    }

    pub fn retry_backoff(&self) -> Duration {
        Duration::from_millis(self.retry_backoff_ms)
    }
}