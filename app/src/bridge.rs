use anyhow::Result;
use moq_net::origin;
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;

pub struct Bridge {
    pub url: String,
    stop: CancellationToken,
    task: JoinHandle<Result<()>>,
}

impl Bridge {
    pub async fn open(publish: origin::Producer, subscribe: origin::Producer) -> Result<Self> {
        let listener = moq_native::websocket::Listener::bind("127.0.0.1:0".parse()?).await?;
        let address = listener.local_addr()?;
        let capability = format!("/{:032x}", rand::random::<u128>());
        let url = format!("ws://{address}{capability}");
        let mut server = moq_native::ServerConfig::default()
            .init()?
            .with_websocket(listener);
        let stop = CancellationToken::new();
        let stopped = stop.clone();
        let task = tokio::spawn(async move {
            let mut sessions = JoinSet::new();

            loop {
                tokio::select! {
                    biased;
                    _ = stopped.cancelled() => break,
                    Some(result) = sessions.join_next() => {
                        if let Err(error) = result {
                            tracing::warn!(%error, "local media session task failed");
                        }
                    }
                    request = server.accept() => {
                        let Some(request) = request else { break };

                        if request.url().is_none_or(|url| url.path() != capability) {
                            request.close(403).await?;
                            continue;
                        }

                        let request = request.with_publisher(&publish).with_subscriber(subscribe.clone());
                        let stopped = stopped.clone();
                        sessions.spawn(async move {
                            match request.ok().await {
                                Ok(session) => {
                                    tokio::select! {
                                        _ = session.closed() => {}
                                        _ = stopped.cancelled() => session.abort(moq_net::Error::Cancel),
                                    }
                                }
                                Err(error) => tracing::warn!(%error, "local media handshake failed"),
                            }
                        });
                    }
                }
            }

            server.close().await;
            sessions.shutdown().await;

            Ok(())
        });

        Ok(Self { url, stop, task })
    }

    pub async fn close(mut self) -> Result<()> {
        self.stop.cancel();
        (&mut self.task).await??;

        Ok(())
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop.cancel();
        self.task.abort();
    }
}
