#[tokio::main]
async fn main() {
    let (mut tx1, mut rx1) = tokio::sync::mpsc::channel::<()>(1);
    let (mut tx2, mut rx2) = tokio::sync::mpsc::channel::<()>(1);

    tokio::select! {
        _ = rx1.recv() => {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(500), async {
                while let Some(_) = rx2.recv().await {
                    println!("rx2 received in arm 1");
                }
            }).await;
        }
        _ = rx2.recv() => {
            println!("rx2 received in arm 2");
        }
    }
}
