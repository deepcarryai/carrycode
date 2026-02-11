use anyhow::Result;
use std::time::Instant;

pub async fn measure_latency(url: &str) -> Result<u64> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let start = Instant::now();
    
    // Use GET request to ensure we hit the server. 
    // Even if it returns 401/404, it means the network path is working.
    let _ = client.get(url).send().await.map_err(|e| anyhow::anyhow!("Request failed: {}", e))?;

    Ok(start.elapsed().as_millis() as u64)
}

pub fn measure_latency_blocking(url: &str) -> Result<u64> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let start = Instant::now();
    
    let _ = client.get(url).send().map_err(|e| anyhow::anyhow!("Request failed: {}", e))?;

    Ok(start.elapsed().as_millis() as u64)
}
