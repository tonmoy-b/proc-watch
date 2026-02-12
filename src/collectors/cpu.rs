use super::*;
use crate::errors::CollectorError;
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::fs;

/// CPU metrics collector that reads directly from /proc/stat.
pub struct CpuCollector {
    prev_sample: Option<CpuSample>,
}

/// Raw CPU tick counts from /proc/stat.
#[derive(Debug, Clone)]
struct CpuSample {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
}

impl CpuSample {
    fn total(&self) -> u64 {
        self.user
            + self.nice
            + self.system
            + self.idle
            + self.iowait
            + self.irq
            + self.softirq
            + self.steal
    }
}

impl CpuCollector {
    pub fn new() -> Self {
        Self { prev_sample: None }
    }

    /// Parse the aggregate CPU line from /proc/stat.
    fn parse_cpu_line(line: &str) -> Result<CpuSample, CollectorError> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            return Err(CollectorError::ParseError {
                path: "/proc/stat".into(),
                field: "cpu".into(),
                raw: line.to_string(),
            });
        }

        let parse = |idx: usize, field: &str| -> Result<u64, CollectorError> {
            parts[idx]
                .parse::<u64>()
                .map_err(|_| CollectorError::ParseError {
                    path: "/proc/stat".into(),
                    field: field.into(),
                    raw: parts[idx].to_string(),
                })
        };

        Ok(CpuSample {
            user: parse(1, "user")?,
            nice: parse(2, "nice")?,
            system: parse(3, "system")?,
            idle: parse(4, "idle")?,
            iowait: parse(5, "iowait")?,
            irq: parse(6, "irq")?,
            softirq: parse(7, "softirq")?,
            steal: parse(8, "steal")?,
        })
    }

    /// Parse /proc/loadavg for load averages.
    fn parse_loadavg(content: &str) -> Result<(f64, f64, f64), CollectorError> {
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(CollectorError::ParseError {
                path: "/proc/loadavg".into(),
                field: "loadavg".into(),
                raw: content.to_string(),
            });
        }

        let parse = |idx: usize, field: &str| -> Result<f64, CollectorError> {
            parts[idx]
                .parse::<f64>()
                .map_err(|_| CollectorError::ParseError {
                    path: "/proc/loadavg".into(),
                    field: field.into(),
                    raw: parts[idx].to_string(),
                })
        };

        Ok((parse(0, "1m")?, parse(1, "5m")?, parse(2, "15m")?))
    }

    /// Count CPU cores from /proc/stat (lines starting with "cpu" followed by a digit).
    fn count_cores(stat_content: &str) -> u32 {
        stat_content
            .lines()
            .filter(|line| {
                line.starts_with("cpu") && line.chars().nth(3).map_or(false, |c| c.is_ascii_digit())
            })
            .count() as u32
    }
}

#[async_trait]
impl Collector for CpuCollector {
    fn name(&self) -> &'static str {
        "cpu"
    }

    async fn collect(&mut self) -> Result<CollectionResult, CollectorError> {
        // Read /proc/stat for CPU ticks
        let stat_content =
            fs::read_to_string("/proc/stat")
                .await
                .map_err(|e| CollectorError::ProcReadError {
                    path: "/proc/stat".into(),
                    source: e,
                })?;

        let cpu_line = stat_content
            .lines()
            .next()
            .ok_or_else(|| CollectorError::ParseError {
                path: "/proc/stat".into(),
                field: "cpu_line".into(),
                raw: "empty file".into(),
            })?;

        let current = Self::parse_cpu_line(cpu_line)?;
        let num_cores = Self::count_cores(&stat_content);

        // Read load averages
        let loadavg_content = fs::read_to_string("/proc/loadavg").await.map_err(|e| {
            CollectorError::ProcReadError {
                path: "/proc/loadavg".into(),
                source: e,
            }
        })?;
        let (load_1m, load_5m, load_15m) = Self::parse_loadavg(&loadavg_content)?;

        // Compute deltas if we have a previous sample
        let (user_pct, system_pct, iowait_pct, idle_pct) = if let Some(ref prev) = self.prev_sample
        {
            let total_delta = current.total().saturating_sub(prev.total());
            if total_delta == 0 {
                (0.0, 0.0, 0.0, 100.0)
            } else {
                let td = total_delta as f64;
                (
                    (current.user.saturating_sub(prev.user)
                        + current.nice.saturating_sub(prev.nice)) as f64
                        / td
                        * 100.0,
                    (current.system.saturating_sub(prev.system)
                        + current.irq.saturating_sub(prev.irq)
                        + current.softirq.saturating_sub(prev.softirq)) as f64
                        / td
                        * 100.0,
                    current.iowait.saturating_sub(prev.iowait) as f64 / td * 100.0,
                    current.idle.saturating_sub(prev.idle) as f64 / td * 100.0,
                )
            }
        } else {
            // First sample — can't compute delta yet.
            // Return zeros; next collection will have real data.
            (0.0, 0.0, 0.0, 0.0)
        };

        self.prev_sample = Some(current);

        let snapshot = CpuSnapshot {
            user_pct,
            system_pct,
            iowait_pct,
            idle_pct,
            num_cores,
            load_avg_1m: load_1m,
            load_avg_5m: load_5m,
            load_avg_15m: load_15m,
        };

        // Determine health status
        let status = if iowait_pct > 30.0 || (100.0 - idle_pct) > 95.0 {
            CheckStatus::Unhealthy
        } else if iowait_pct > 10.0 || (100.0 - idle_pct) > 80.0 {
            CheckStatus::Degraded
        } else {
            CheckStatus::Healthy
        };

        let message = format!(
            "user={:.1}% sys={:.1}% iowait={:.1}% idle={:.1}% load={:.2}",
            user_pct, system_pct, iowait_pct, idle_pct, load_1m,
        );

        Ok(CollectionResult {
            check_name: self.name().to_string(),
            status,
            message,
            metadata: HashMap::new(),
            latency_us: 0, // filled by timed_collect wrapper
            payload: MetricPayload::Cpu(snapshot),
        })
    }
}

// ─────────────────────────────────────────────
// Unit tests — validate on hardcoded information
// ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_STAT: &str = "\
cpu  10132153 290696 3084719 46828483 16683 0 25195 0 0 0
cpu0 1393280 32966 572056 13343292 6130 0 17875 0 0 0
cpu1 1335498 35507 523368 13200746 4990 0 3670 0 0 0";

    #[test]
    fn test_parse_cpu_line() {
        let line = "cpu  10132153 290696 3084719 46828483 16683 0 25195 0 0 0";
        let sample = CpuCollector::parse_cpu_line(line).unwrap();
        assert_eq!(sample.user, 10132153);
        assert_eq!(sample.nice, 290696);
        assert_eq!(sample.system, 3084719);
        assert_eq!(sample.idle, 46828483);
        assert_eq!(sample.iowait, 16683);
        assert_eq!(sample.steal, 0);
    }

    #[test]
    fn test_parse_cpu_line_too_short() {
        let line = "cpu  100 200";
        assert!(CpuCollector::parse_cpu_line(line).is_err());
    }

    #[test]
    fn test_count_cores() {
        assert_eq!(CpuCollector::count_cores(SAMPLE_STAT), 2);
    }

    #[test]
    fn test_parse_loadavg() {
        let content = "0.50 0.75 1.00 2/1234 5678";
        let (l1, l5, l15) = CpuCollector::parse_loadavg(content).unwrap();
        assert!((l1 - 0.50).abs() < f64::EPSILON);
        assert!((l5 - 0.75).abs() < f64::EPSILON);
        assert!((l15 - 1.00).abs() < f64::EPSILON);
    }

    #[test]
    fn test_delta_computation() {
        let prev = CpuSample {
            user: 1000,
            nice: 0,
            system: 500,
            idle: 8000,
            iowait: 100,
            irq: 0,
            softirq: 0,
            steal: 0,
        };
        let curr = CpuSample {
            user: 1200,
            nice: 0,
            system: 600,
            idle: 8100,
            iowait: 100,
            irq: 0,
            softirq: 0,
            steal: 0,
        };
        let total_delta = curr.total() - prev.total();
        assert_eq!(total_delta, 400);

        let user_pct = (curr.user - prev.user) as f64 / total_delta as f64 * 100.0;
        assert!((user_pct - 50.0).abs() < f64::EPSILON);
    }
}
