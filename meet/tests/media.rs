use std::time::Duration;

use anyhow::{Context, Result};
use iroh::{Endpoint, endpoint::presets};
use moq_net::{Timestamp, broadcast};
use zero_meet::{
    client::{self, RoomConnection},
    protocol::{MediaState, media_origin, media_path},
    rooms::Rooms,
    server::Server,
};

#[tokio::test]
async fn authorized_forwarding_isolates_rooms_and_revokes_on_leave() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(15), async {
        let server = Server::spawn(Endpoint::bind(presets::Minimal).await?);
        let alice = Endpoint::bind(presets::Minimal).await?;
        let bob = Endpoint::bind(presets::Minimal).await?;
        let eve = Endpoint::bind(presets::Minimal).await?;
        let invite = client::create(&alice, server.router.endpoint().addr()).await?;
        let a =
            RoomConnection::join(&alice, &invite, "Alice".into(), MediaState::default()).await?;
        let b = RoomConnection::join(&bob, &invite, "Bob".into(), MediaState::default()).await?;
        let other_invite = client::create(&eve, server.router.endpoint().addr()).await?;
        let e =
            RoomConnection::join(&eve, &other_invite, "Eve".into(), MediaState::default()).await?;
        let outgoing = media_origin();
        let incoming = media_origin();
        let eve_incoming = media_origin();
        let mut drivers = tokio::task::JoinSet::new();

        // Eve cannot reuse Alice's generation even with the correct room capability.
        let unauthorized = client::connect_media(
            &eve,
            &invite,
            &a.generation,
            &media_origin(),
            media_origin(),
        )
        .await;
        match unauthorized {
            Err(_) => {}
            Ok((session, driver)) => {
                drivers.spawn(driver);
                assert!(matches!(
                    session.closed().await,
                    moq_net::Error::Unauthorized | moq_net::Error::Transport(_)
                ));
            }
        }

        let (a_media, driver) =
            client::connect_media(&alice, &invite, &a.generation, &outgoing, media_origin())
                .await?;
        drivers.spawn(driver);
        let (_b_media, driver) = client::connect_media(
            &bob,
            &invite,
            &b.generation,
            &media_origin(),
            incoming.clone(),
        )
        .await?;
        drivers.spawn(driver);
        let (_e_media, driver) = client::connect_media(
            &eve,
            &other_invite,
            &e.generation,
            &media_origin(),
            eve_incoming.clone(),
        )
        .await?;
        drivers.spawn(driver);
        let name = a.roster[0].broadcast.clone();
        let mut broadcast = outgoing.create_broadcast(&name, broadcast::Route::announced())?;
        let mut track = broadcast.create_track("audio", None)?;
        let received = incoming
            .consume()
            .announced_broadcast(&name)
            .await
            .context("announcement missing")?;
        let mut read = received.track("audio")?.subscribe(None).await?;
        track.write_frame(Timestamp::now(), b"encoded opus frame".as_slice())?;
        let frame = read.read_frame().await?.context("frame missing")?;
        assert_eq!(frame.payload.as_ref(), b"encoded opus frame");

        let isolated = tokio::time::timeout(
            Duration::from_millis(150),
            eve_incoming.consume().announced_broadcast(&name),
        )
        .await;
        assert!(isolated.is_err(), "other room received an announcement");
        drop(a);
        let _ = a_media.closed().await;
        if let Ok((stale, driver)) =
            client::connect_media(&alice, &invite, "stale", &outgoing, media_origin()).await
        {
            drivers.spawn(driver);
            let _ = stale.closed().await;
        }

        drop(b);
        drop(e);
        drivers.shutdown().await;
        server.close().await?;
        alice.close().await;
        bob.close().await;
        eve.close().await;

        Ok::<_, anyhow::Error>(())
    })
    .await??;

    Ok(())
}

#[tokio::test]
async fn publication_scope_cannot_escape_its_participant_generation() {
    let rooms = Rooms::default();
    let room = rooms.create();
    let peer = iroh::SecretKey::from_bytes(&rand::random()).public();
    let membership = rooms
        .join(
            peer,
            zero_meet::protocol::JoinRoom {
                room,
                name: "Alice".into(),
                media: MediaState::default(),
            },
        )
        .unwrap();
    let access = rooms
        .authorize(peer, &media_path(room, &membership.generation))
        .unwrap();
    let own = membership.roster.borrow()[0].broadcast.clone();
    assert!(
        access
            .subscribe
            .create_broadcast(&own, broadcast::Route::announced())
            .is_ok()
    );
    assert!(
        access
            .subscribe
            .create_broadcast("someone-else/camera.hang", broadcast::Route::announced())
            .is_err()
    );
    assert!(
        access
            .subscribe
            .create_broadcast(
                format!("{peer}/other-generation/camera.hang"),
                broadcast::Route::announced()
            )
            .is_err()
    );
    assert!(rooms.authorize(peer, "/bad/path").is_none());
}
