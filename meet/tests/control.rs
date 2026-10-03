use std::time::Duration;

use anyhow::Result;
use iroh::{Endpoint, endpoint::presets};
use zero_meet::{
    client::{self, RoomConnection},
    protocol::{MediaState, Rejection, RoomEvent},
    server::Server,
};

#[tokio::test]
async fn actual_irpc_roster_updates_disconnect_and_restart() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(15), async {
        let endpoint = Endpoint::bind(presets::Minimal).await?;
        let key = endpoint.secret_key().clone();
        let server = Server::spawn(endpoint);
        let alice = Endpoint::bind(presets::Minimal).await?;
        let bob = Endpoint::bind(presets::Minimal).await?;
        let invite = client::create(&alice, server.router.endpoint().addr()).await?;
        let mut a = RoomConnection::join(&alice, &invite, "Alice".into(), MediaState::default()).await?;
        assert_eq!(a.roster.len(), 1);
        let b = RoomConnection::join(&bob, &invite, "Bob".into(), MediaState::default()).await?;
        assert_eq!(b.roster.len(), 2);
        assert!(matches!(a.events.recv().await?, Some(RoomEvent::Roster(roster)) if roster.len() == 2));

        b.updates.send(MediaState { microphone: true, camera: false }).await?;
        assert!(matches!(a.events.recv().await?, Some(RoomEvent::Roster(roster)) if roster[1].media.microphone));
        drop(b);
        assert!(matches!(a.events.recv().await?, Some(RoomEvent::Roster(roster)) if roster.len() == 1));
        drop(a);
        server.close().await?;

        let endpoint = Endpoint::builder(presets::Minimal).secret_key(key).bind().await?;
        let restarted = Server::spawn(endpoint);
        let mut stale = invite;
        stale.server = restarted.router.endpoint().addr();
        let result = RoomConnection::join(&bob, &stale, "Bob".into(), MediaState::default()).await;
        assert_eq!(result.err().unwrap().downcast_ref::<Rejection>(), Some(&Rejection::Expired));
        restarted.close().await?;
        alice.close().await;
        bob.close().await;

        Ok::<_, anyhow::Error>(())
    }).await??;

    Ok(())
}
