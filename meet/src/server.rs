use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::Path,
    time::Duration,
};

use anyhow::{Context, Result};
use iroh::{
    Endpoint, SecretKey,
    endpoint::Connection,
    protocol::{AcceptError, ProtocolHandler, Router},
};
use irpc::WithChannels;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::{
    protocol::{ALPN, MeetMessage, MeetProtocol, RoomEvent},
    rooms::Rooms,
};

pub struct Server {
    pub router: Router,
    stop: CancellationToken,
    expiry: JoinHandle<()>,
}

impl Server {
    pub fn spawn(endpoint: Endpoint) -> Self {
        let rooms = Rooms::default();
        let router = Router::builder(endpoint)
            .accept(ALPN, Control(rooms.clone()))
            .accept(iroh_moq::ALPN, Media(rooms.clone()))
            .spawn();
        let stop = CancellationToken::new();
        let stopped = stop.clone();
        let expiry = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                tokio::select! {
                    _ = stopped.cancelled() => break,
                    _ = interval.tick() => rooms.expire(),
                }
            }
        });

        Self {
            router,
            stop,
            expiry,
        }
    }

    pub async fn close(mut self) -> Result<()> {
        self.stop.cancel();
        self.router.shutdown().await?;
        (&mut self.expiry).await?;

        Ok(())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.cancel();
        self.expiry.abort();
    }
}

#[derive(Debug, Clone)]
struct Control(Rooms);

impl ProtocolHandler for Control {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        while let Some(message) = irpc_iroh::read_request::<MeetProtocol>(&connection).await? {
            match message {
                MeetMessage::Create(request) => {
                    request.tx.send(self.0.create()).await.ok();
                }
                MeetMessage::Join(request) => {
                    let WithChannels {
                        inner, tx, mut rx, ..
                    } = request;
                    let mut membership = match self.0.join(connection.remote_id(), inner) {
                        Ok(membership) => membership,
                        Err(reason) => {
                            tx.send(RoomEvent::Rejected(reason)).await.ok();
                            // QUIC close can discard queued stream data before the rejection arrives.
                            let _ =
                                tokio::time::timeout(Duration::from_secs(5), connection.closed())
                                    .await;
                            break;
                        }
                    };
                    let roster = membership.roster.borrow_and_update().clone();
                    let joined = RoomEvent::Joined {
                        generation: membership.generation.clone(),
                        roster,
                    };

                    if !send_event(&tx, joined).await {
                        break;
                    }

                    loop {
                        tokio::select! {
                            biased;
                            _ = membership.revoked.cancelled() => break,
                            media = rx.recv() => match media {
                                Ok(Some(media)) => membership.update(media),
                                _ => break,
                            },
                            changed = membership.roster.changed() => {
                                if changed.is_err() { break; }
                                let roster = membership.roster.borrow_and_update().clone();
                                if !send_event(&tx, RoomEvent::Roster(roster)).await { break; }
                            }
                        }
                    }

                    break;
                }
            }
        }

        connection.close(0_u32.into(), b"control ended");

        Ok(())
    }
}

async fn send_event(tx: &irpc::channel::mpsc::Sender<RoomEvent>, event: RoomEvent) -> bool {
    matches!(
        tokio::time::timeout(Duration::from_secs(5), tx.send(event)).await,
        Ok(Ok(()))
    )
}

#[derive(Debug, Clone)]
struct Media(Rooms);

impl ProtocolHandler for Media {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        let peer = connection.remote_id();
        let transport = web_transport_iroh::Session::raw(connection);
        let request = tokio::time::timeout(
            Duration::from_secs(5),
            moq_net::Server::new().accept_request(transport),
        )
        .await
        .map_err(AcceptError::from_err)?
        .map_err(AcceptError::from_err)?;
        let Some(access) = self.0.authorize(peer, request.path()) else {
            request.close(moq_net::Error::Unauthorized);

            return Ok(());
        };
        let (session, driver) = request
            .with_publisher(&access.publish)
            .with_subscriber(access.subscribe)
            .ok()
            .await
            .map_err(AcceptError::from_err)?;

        tokio::select! {
            biased;
            _ = access.revoked.cancelled() => session.abort(moq_net::Error::Cancel),
            result = driver => {
                if let Err(error) = result {
                    tracing::debug!(%peer, %error, "media connection ended");
                }
            }
        }

        Ok(())
    }
}

pub fn load_key(path: &Path) -> Result<SecretKey> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    match options.open(path) {
        Ok(mut file) => {
            let bytes: [u8; 32] = rand::random();
            file.write_all(&bytes).context("write server identity")?;
            file.sync_all().context("persist server identity")?;

            Ok(SecretKey::from_bytes(&bytes))
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(33)
                .read_to_end(&mut bytes)?;
            let bytes: [u8; 32] = bytes
                .try_into()
                .map_err(|_| anyhow::anyhow!("server identity must contain exactly 32 bytes"))?;

            Ok(SecretKey::from_bytes(&bytes))
        }
        Err(error) => Err(error).context("create server identity"),
    }
}
