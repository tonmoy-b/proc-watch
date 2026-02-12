use super::*;
use crate::errors::CollectorError;
use super::{Collector, CollectionResult, CheckStatus, MetricPayload, MemorySnapshot};
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::fs;

/// Memory metrics collector reading directly from /proc/meminfo.
pub struct MemoryCollector;

impl MemoryCollector {
    pub fn new() -> Self {
        Self
    }

    /// Parse /proc/meminfo into a key-value map of kB values.
    fn parse_meminfo(content: &str) -> Result<HashMap<String, u64>, CollectorError> {
        let mut map = HashMap::new();
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let key = parts[0].trim_end_matches(':').to_string();
                if let Ok(val) = parts[1].parse::<u64>() {
                    map.insert(key, val);
                }
            }
        }
        Ok(map)
    }

    /// Extract a required field from meminfo, converting kB -> bytes.
    fn get_bytes(
        map: &HashMap<String, u64>,
        field: &str,
    ) -> Result<u64, CollectorError> {
        map.get(field)
            .map(|kb| kb * 1024)
            .ok_or_else(|| CollectorError::ParseError {
                path: "/proc/meminfo".into(),
                field: field.into(),
                raw: "field not found".into(),
            })
    }
}

#[async_trait]
impl Collector for MemoryCollector {
    fn name(&self) -> &'static str {
        "memory"
    }

    async fn collect(&mut self) -> Result<CollectionResult, CollectorError> {
        let content = fs::read_to_string("/proc/meminfo")
            .await
            .map_err(|e| CollectorError::ProcReadError {
                path: "/proc/meminfo".into(),
                source: e,
            })?;

        let map = Self::parse_meminfo(&content)?;

        let total = Self::get_bytes(&map, "MemTotal")?;
        let available = Self::get_bytes(&map, "MemAvailable")?;
        let swap_total = Self::get_bytes(&map, "SwapTotal").unwrap_or(0);
        let swap_free = Self::get_bytes(&map, "SwapFree").unwrap_or(0);

        let used = total.saturating_sub(available);
        let swap_used = swap_total.saturating_sub(swap_free);
        let pressure_pct = if total > 0 {
            used as f64 / total as f64 * 100.0
        } else {
            0.0
        };

        let snapshot = MemorySnapshot {
            total_bytes: total,
            available_bytes: available,
            used_bytes: used,
            swap_total_bytes: swap_total,
            swap_used_bytes: swap_used,
            memory_pressure_pct: pressure_pct,
        };

        let status = if pressure_pct > 95.0
            || (swap_total > 0 && swap_used > swap_total * 80 / 100)
        {
            CheckStatus::Unhealthy
        } else if pressure_pct > 80.0 {
            CheckStatus::Degraded
        } else {
            CheckStatus::Healthy
        };

        let message = format!(
            "used={:.1}% ({}/{} MB) swap={}/{} MB",
            pressure_pct,
            used / (1024 * 1024),
            total / (1024 * 1024),
            swap_used / (1024 * 1024),
            swap_total / (1024 * 1024),
        );

        Ok(CollectionResult {
            check_name: self.name().to_string(),
            status,
            message,
            metadata: HashMap::new(),
            latency_us: 0,
            payload: MetricPayload::Memory(snapshot),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MEMINFO: &str = "\
MemTotal:       16384000 kB
MemFree:         2048000 kB
MemAvailable:    4096000 kB
Buffers:          512000 kB
Cached:          2048000 kB
SwapTotal:       8192000 kB
SwapFree:        4096000 kB";

    #[test]
    fn test_parse_meminfo() {
        let map = MemoryCollector::parse_meminfo(SAMPLE_MEMINFO).unwrap();
        assert_eq!(map["MemTotal"], 16384000);
        assert_eq!(map["MemAvailable"], 4096000);
        assert_eq!(map["SwapTotal"], 8192000);
        assert_eq!(map["SwapFree"], 4096000);
    }

    #[test]
    fn test_get_bytes_converts_kb_to_bytes() {
        let mut map = HashMap::new();
        map.insert("MemTotal".to_string(), 1024);
        let bytes = MemoryCollector::get_bytes(&map, "MemTotal").unwrap();
        assert_eq!(bytes, 1024 * 1024);
    }

    #[test]
    fn test_get_bytes_missing_field() {
        let map = HashMap::new();
        assert!(MemoryCollector::get_bytes(&map, "NonExistent").is_err());
    }

    #[test]
    fn test_pressure_calculation() {
        let total: u64 = 16384000 * 1024;
        let available: u64 = 4096000 * 1024;
        let used = total - available;
        let pct = used as f64 / total as f64 * 100.0;
        assert!((pct - 75.0).abs() < 0.01);
    }
}