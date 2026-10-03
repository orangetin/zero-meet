#[path = "../src/bridge.rs"]
mod bridge;

use std::time::Duration;

use anyhow::{Context, Result};
use iroh::{Endpoint, endpoint::presets};
use moq_net::{Timestamp, broadcast};
use tokio::task::JoinSet;
use zero_meet::{
    client::{self, RoomConnection},
    protocol::{MediaState, media_origin},
    server::Server,
};

use bridge::Bridge;

#[tokio::test]
async fn media_crosses_both_desktop_bridges_and_the_room_server() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let server = Server::spawn(Endpoint::bind(presets::Minimal).await?);
        let mut endpoints = Vec::new();
        let mut controls = Vec::new();
        let mut media_sessions = Vec::new();
        let mut bridges = Vec::new();
        let mut browser_sessions = Vec::new();
        let mut sources = Vec::new();
        let mut sinks = Vec::new();
        let mut drivers = JoinSet::new();
        let creator = Endpoint::bind(presets::Minimal).await?;
        let invite = client::create(&creator, server.router.endpoint().addr()).await?;

        for name in ["Alice", "Bob"] {
            let endpoint = Endpoint::bind(presets::Minimal).await?;
            let control =
                RoomConnection::join(&endpoint, &invite, name.into(), MediaState::default())
                    .await?;
            let outgoing = media_origin();
            let incoming = media_origin();
            let (media, driver) = client::connect_media(
                &endpoint,
                &invite,
                &control.generation,
                &outgoing,
                incoming.clone(),
            )
            .await?;
            drivers.spawn(driver);
            let bridge = Bridge::open(incoming, outgoing).await?;
            let source = media_origin();
            let sink = media_origin();
            let browser = moq_native::ClientConfig::default()
                .init()?
                .with_publisher(&source)
                .with_subscriber(sink.clone())
                .connect(bridge.url.parse()?)
                .await?;
            endpoints.push(endpoint);
            controls.push(control);
            media_sessions.push(media);
            bridges.push(bridge);
            browser_sessions.push(browser);
            sources.push(source);
            sinks.push(sink);
        }

        for sender in 0..2 {
            let name = controls[sender]
                .roster
                .iter()
                .find(|person| person.id == endpoints[sender].id().to_string())
                .unwrap()
                .broadcast
                .clone();
            let mut published =
                sources[sender].create_broadcast(&name, broadcast::Route::announced())?;
            let mut track = published.create_track("audio", None)?;
            let received = sinks[1 - sender]
                .consume()
                .announced_broadcast(&name)
                .await
                .context("remote announcement missing")?;
            let mut reader = received.track("audio")?.subscribe(None).await?;
            track.write_frame(
                Timestamp::now(),
                b"through browser bridge and iroh".as_slice(),
            )?;
            let frame = reader.read_frame().await?.context("remote frame missing")?;
            assert_eq!(frame.payload.as_ref(), b"through browser bridge and iroh");
            track.finish()?;
        }

        drop(browser_sessions);

        for bridge in bridges {
            bridge.close().await?;
        }

        drop(media_sessions);
        drivers.shutdown().await;
        drop(controls);

        for endpoint in endpoints {
            endpoint.close().await;
        }

        creator.close().await;
        server.close().await?;

        Ok::<_, anyhow::Error>(())
    })
    .await??;

    Ok(())
}
