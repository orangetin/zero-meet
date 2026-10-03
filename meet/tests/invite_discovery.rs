use std::time::Duration;

use anyhow::Result;
use iroh::{Endpoint, endpoint::presets};
use zero_meet::{
    client::{self, RoomConnection},
    protocol::{Invite, MediaState},
    server::Server,
};

#[tokio::test]
#[ignore = "requires the public iroh discovery and relay services"]
async fn fresh_guest_joins_with_only_a_shared_invitation() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(45), async {
        let server = Server::spawn(Endpoint::bind(presets::N0).await?);
        server.router.endpoint().online().await;

        let address = server.router.endpoint().id().to_string();
        assert_eq!(address.len(), 64);
        let server_id = address.parse::<iroh::EndpointId>()?;

        let host = Endpoint::bind(presets::N0).await?;
        let invite = client::create(&host, server_id.into()).await?;
        let shared = Invite::parse(&invite.link())?;
        host.close().await;

        // A new guest has no cached route or direct server address.
        let guest = Endpoint::bind(presets::N0).await?;
        let room =
            RoomConnection::join(&guest, &shared, "Guest".into(), MediaState::default()).await?;
        assert_eq!(room.roster.len(), 1);

        drop(room);
        guest.close().await;
        server.close().await?;

        Ok::<_, anyhow::Error>(())
    })
    .await??;

    Ok(())
}
