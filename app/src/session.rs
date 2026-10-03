use std::time::Duration;

use anyhow::{Context, Result};
use iroh::{Endpoint, endpoint::presets};
use serde::Serialize;
use tauri::ipc::Channel;
use tokio::{
    sync::{Mutex, OnceCell, watch},
    task::JoinHandle,
    time::Instant,
};
use tokio_util::sync::CancellationToken;
use zero_meet::{
    client::{self, RoomConnection},
    protocol::{Invite, MediaState, Participant, Rejection, RoomEvent, media_origin},
};

use crate::bridge::Bridge;

#[derive(Default)]
pub struct AppState {
    endpoint: OnceCell<Endpoint>,
    pub active: Mutex<Option<Call>>,
}

impl AppState {
    pub async fn endpoint(&self) -> Result<&Endpoint> {
        self.endpoint
            .get_or_try_init(|| Endpoint::bind(presets::N0))
            .await
            .map_err(Into::into)
    }

    pub async fn close(&self) {
        if let Some(call) = self.active.lock().await.take() {
            call.close().await;
        }

        if let Some(endpoint) = self.endpoint.get() {
            endpoint.close().await;
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    Connected {
        url: String,
        me: String,
        broadcast: String,
        roster: Vec<Participant>,
    },
    Roster {
        roster: Vec<Participant>,
    },
    Latency {
        milliseconds: Option<u64>,
    },
    Reconnecting,
    Failed {
        message: String,
        retryable: bool,
    },
}

pub struct Call {
    pub media: watch::Sender<MediaState>,
    stop: CancellationToken,
    task: JoinHandle<()>,
}

impl Call {
    pub fn start(
        endpoint: Endpoint,
        invite: Invite,
        name: String,
        media: MediaState,
        events: Channel<Event>,
    ) -> Self {
        let (media, updates) = watch::channel(media);
        let stop = CancellationToken::new();
        let stopped = stop.clone();
        let task = tokio::spawn(async move {
            let mut deadline = Instant::now() + Duration::from_secs(30);
            let mut delay = Duration::from_millis(250);

            loop {
                let result = run_attempt(
                    &endpoint,
                    &invite,
                    &name,
                    updates.clone(),
                    &events,
                    &stopped,
                    deadline,
                )
                .await;
                let error = match result {
                    Ok(()) => break,
                    Err(error) => error,
                };

                if error.downcast_ref::<Rejection>().is_some() {
                    let _ = events.send(Event::Failed {
                        message: error.to_string(),
                        retryable: false,
                    });
                    break;
                }

                if error
                    .downcast_ref::<ConnectedFailure>()
                    .is_some_and(|failure| failure.healthy)
                {
                    deadline = Instant::now() + Duration::from_secs(30);
                    delay = Duration::from_millis(250);
                }

                if Instant::now() >= deadline {
                    let _ = events.send(Event::Failed {
                        message: "Connection lost. Check your network and rejoin.".into(),
                        retryable: true,
                    });
                    break;
                }

                if events.send(Event::Reconnecting).is_err() {
                    break;
                }

                tokio::select! {
                    _ = stopped.cancelled() => break,
                    _ = tokio::time::sleep(delay.min(deadline.saturating_duration_since(Instant::now()))) => {}
                }
                delay = (delay * 2).min(Duration::from_secs(2));
            }
        });

        Self { media, stop, task }
    }

    pub async fn close(mut self) {
        self.stop.cancel();
        let _ = (&mut self.task).await;
    }
}

impl Drop for Call {
    fn drop(&mut self) {
        self.stop.cancel();
        self.task.abort();
    }
}

#[derive(Debug)]
struct ConnectedFailure {
    healthy: bool,
}

impl std::fmt::Display for ConnectedFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("meeting connection ended")
    }
}

impl std::error::Error for ConnectedFailure {}

async fn run_attempt(
    endpoint: &Endpoint,
    invite: &Invite,
    name: &str,
    mut updates: watch::Receiver<MediaState>,
    events: &Channel<Event>,
    stopped: &CancellationToken,
    deadline: Instant,
) -> Result<()> {
    let initial = *updates.borrow_and_update();
    let establish = async {
        let control = tokio::time::timeout(
            Duration::from_secs(5),
            RoomConnection::join(endpoint, invite, name.to_owned(), initial),
        )
        .await??;
        let outgoing = media_origin();
        let incoming = media_origin();
        let (media, driver) = tokio::time::timeout(
            Duration::from_secs(5),
            client::connect_media(
                endpoint,
                invite,
                &control.generation,
                &outgoing,
                incoming.clone(),
            ),
        )
        .await??;
        let bridge = Bridge::open(incoming, outgoing).await?;
        Ok::<_, anyhow::Error>((control, media, driver, bridge))
    };
    let (mut control, media, driver, bridge) = tokio::select! {
        biased;
        _ = stopped.cancelled() => return Ok(()),
        result = tokio::time::timeout_at(deadline, establish) => result??,
    };
    let connected = Instant::now();
    let me = endpoint.id().to_string();
    let broadcast = control
        .roster
        .iter()
        .find(|participant| participant.id == me)
        .context("own participant missing")?
        .broadcast
        .clone();
    events.send(Event::Connected {
        url: bridge.url.clone(),
        me,
        broadcast,
        roster: control.roster.clone(),
    })?;
    tokio::pin!(driver);
    let mut latency_tick = tokio::time::interval(Duration::from_secs(2));
    latency_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            biased;
            _ = stopped.cancelled() => break,
            _ = &mut driver => break,
            _ = latency_tick.tick() => {
                let milliseconds = control.server_rtt().map(|rtt| rtt.as_millis() as u64);

                if events.send(Event::Latency { milliseconds }).is_err() {
                    break;
                }
            }
            result = updates.changed() => {
                if result.is_err() { break; }
                let state = *updates.borrow_and_update();
                if !matches!(tokio::time::timeout(Duration::from_secs(2), control.updates.send(state)).await, Ok(Ok(()))) { break; }
            }
            event = control.events.recv() => match event {
                Ok(Some(RoomEvent::Roster(roster))) => {
                    if events.send(Event::Roster { roster }).is_err() { break; }
                }
                _ => break,
            },
        }
    }

    media.abort(moq_net::Error::Cancel);
    bridge.close().await?;

    if stopped.is_cancelled() {
        return Ok(());
    }

    Err(ConnectedFailure {
        healthy: connected.elapsed() >= Duration::from_secs(2),
    }
    .into())
}
