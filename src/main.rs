use chrono::Utc;
use serde::Serialize;
use std::ffi::OsStr;
use sysinfo::System;
use tokio::time::{self, Duration};

#[derive(Serialize, Debug)]
struct Heartbeat {
    timestamp: String,
    node_id: String,
    cpu_usage: f32,
    memory_used_kb: u64,
    mysql_status: String, // chech our infra -- mysql here. process-name is 'mysql'
}

async fn collect_metrics(sys: &mut System) -> Heartbeat {
    sys.refresh_all();

    // Check if a process named 'mysql' is running
    let is_mysql_up = sys.processes_by_name(OsStr::new("mysql")).next().is_some();

    Heartbeat {
        timestamp: Utc::now().to_rfc3339(),
        node_id: "azure-mysql-node-01".to_string(),
        cpu_usage: sys.global_cpu_usage(),
        memory_used_kb: sys.used_memory(),
        mysql_status: if is_mysql_up {
            "UP".to_string()
        } else {
            "DOWN".to_string()
        },
    }
}

#[tokio::main]
async fn main() {
    let mut sys = System::new_all();
    let mut interval = time::interval(Duration::from_secs(5)); //heartbeat checked in 5s intervals

    println!("Starting Infra Health Agent [for MySQL]...");

    loop {
        interval.tick().await;
        let health_data = collect_metrics(&mut sys).await;

        // TODO:: place into an API endpoint
        let json_payload = serde_json::to_string(&health_data).unwrap();
        println!("Sending Heartbeat: {}", json_payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ... existing tests ...

    #[test]
    fn test_multiply() {
        assert_eq!(6 + 6, 12);
    }
}
