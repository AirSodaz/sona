use tauri_appsona_lib::process_download;
use std::time::{Duration, Instant};
use futures_util::stream;
use bytes::Bytes;
use tokio::io::AsyncReadExt;

#[tokio::test(flavor = "current_thread")]
async fn test_async_behavior() {
    // 1. Establish Baseline for Canary
    // See how many ticks the canary gets when running alone for 200ms.
    let control_canary = tokio::spawn(async move {
        let mut ticks = 0;
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(200) {
            tokio::time::sleep(Duration::from_millis(1)).await;
            ticks += 1;
        }
        ticks
    });
    let control_ticks = control_canary.await.unwrap();
    println!("Control ticks (canary alone): {}", control_ticks);


    // 2. Test with Download (Async Backpressure)
    let (mut client, server) = tokio::io::duplex(10);
    let chunks: Vec<Result<Bytes, String>> = (0..20)
        .map(|_| Ok(Bytes::from_static(&[0u8; 10])))
        .collect();
    let stream = stream::iter(chunks);

    // Spawn a slow reader (backpressure source)
    tokio::spawn(async move {
        let mut buf = [0u8; 10];
        loop {
            tokio::time::sleep(Duration::from_millis(10)).await;
            if client.read(&mut buf).await.unwrap() == 0 {
                break;
            }
        }
    });

    // Spawn concurrent canary
    let canary = tokio::spawn(async move {
        let mut ticks = 0;
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(200) {
            tokio::time::sleep(Duration::from_millis(1)).await;
            ticks += 1;
        }
        ticks
    });

    process_download(stream, server, 0, |_, _| {}).await.unwrap();
    let ticks = canary.await.unwrap();
    println!("Test ticks (concurrent with download): {}", ticks);

    // Compare
    let ratio = ticks as f64 / control_ticks as f64;
    println!("Efficiency Ratio: {:.2}", ratio);

    if ratio > 0.8 {
        println!("Confirmed: No significant starvation (ratio > 0.8).");
    } else {
        println!("Warning: Starvation detected.");
    }
    assert!(ratio > 0.8, "Task starvation detected! Ratio: {}", ratio);
}
