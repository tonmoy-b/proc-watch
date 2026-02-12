use infra_health_agent::collectors::cpu::CpuCollector;
use infra_health_agent::collectors::memory::MemoryCollector;
use infra_health_agent::collectors::Collector;
use infra_health_agent::errors::CollectorError;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Test memory collector
    let mut mem = MemoryCollector::new();
    match mem.collect().await {
        Ok(result) => println!("Memory: {:?}\n", result),
        Err(e) => eprintln!("Memory error: {}", e),
    }

    // Test CPU collector - needs two samples
    let mut cpu = CpuCollector::new();

    // First call seeds the baseline
    let _ = cpu.collect().await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // call #2
    match cpu.collect().await {
        Ok(result) => println!("CPU: {:?}\n", result),
        Err(CollectorError::ParseError { field, path, .. }) => {
            eprintln!("Failed to parse {} in {}", field, path);
        }
        Err(e) => eprintln!("Other error: {}", e),
    }

    Ok(())
}
