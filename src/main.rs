use msp_client::{BrowserBrand, MspClient};
use msp_client::pending_random_daily_children;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    tracing_subscriber::fmt()
        .with_env_filter("msp_client=debug,wreq=info")
        .init();

    std::future::pending::<()>().await;
    Ok(())
}