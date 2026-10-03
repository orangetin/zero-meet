use anyhow::Result;
use iroh::{Endpoint, endpoint::presets};
use zero_meet::server::{Server, load_key};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let path = std::env::args_os()
        .nth(1)
        .unwrap_or_else(|| "server.key".into());
    let key = load_key(std::path::Path::new(&path))?;

    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(key)
        .bind()
        .await?;

    let server = Server::spawn(endpoint);
    server.router.endpoint().online().await;

    println!("Server address: {}", server.router.endpoint().id());

    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result?,
        _ = terminate.recv() => {}
    }

    server.close().await
}
