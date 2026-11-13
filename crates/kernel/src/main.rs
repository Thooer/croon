//! Kernel 的守护进程。目前只验证空壳：配好依赖、起进程、能收信号退出。

use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "croon kernel 启动");

    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => tracing::info!("收到中断"),
        _ = interrupt.recv() => tracing::info!("收到终止"),
        _ = tokio::time::sleep(Duration::from_secs(3600)) => tracing::info!("超时退出"),
    }

    tracing::info!("退出");
    Ok(())
}
