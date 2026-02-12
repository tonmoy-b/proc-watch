pub mod cpu;
pub mod memory;

use crate::errors::CollectorError;
use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;

#[async_trait]
pub trait Collector: Send + Sync {
    /// name of the collector as used in reports
    fn name(&self) -> &'static str;

    /// gather collection then return structured metrics.
    async fn collect(&mut self) -> Result<CollectionResult, CollectorError>;
}

/// result from any collector.
#[derive(Debug, Clone, Serialize)]
pub struct CollectionResult {
    pub check_name: String,
    pub status: CheckStatus,
    pub message: String,
    pub metadata: HashMap<String, String>,
    pub latency_us: u64,
    pub payload: MetricPayload,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum MetricPayload {
    Cpu(CpuSnapshot),
    Memory(MemorySnapshot),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CheckStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize)]
pub struct CpuSnapshot {
    pub user_pct: f64,
    pub system_pct: f64,
    pub iowait_pct: f64,
    pub idle_pct: f64,
    pub num_cores: u32,
    pub load_avg_1m: f64,
    pub load_avg_5m: f64,
    pub load_avg_15m: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemorySnapshot {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub memory_pressure_pct: f64,
}