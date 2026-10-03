use anyhow::{Context, Result};
use iroh::{Endpoint, EndpointAddr, endpoint::Connection};
use irpc::channel::mpsc;

use crate::protocol::{
    ALPN, CreateRoom, Invite, JoinRoom, MediaState, MeetProtocol, Participant, RoomEvent,
};

pub struct RoomConnection {
    connection: Connection,
    pub updates: mpsc::Sender<MediaState>,
    pub events: mpsc::Receiver<RoomEvent>,
    pub generation: String,
    pub roster: Vec<Participant>,
}

pub async fn create(endpoint: &Endpoint, server: EndpointAddr) -> Result<Invite> {
    let connection = endpoint.connect(server.clone(), ALPN).await?;
    let api = irpc::Client::<MeetProtocol>::boxed(irpc_iroh::IrohRemoteConnection::new(
        connection.clone(),
    ));
    let result = api.rpc(CreateRoom).await;
    connection.close(0_u32.into(), b"created");

    Ok(Invite {
        server,
        room: result?,
    })
}

impl RoomConnection {
    pub fn server_rtt(&self) -> Option<std::time::Duration> {
        self.connection
            .paths()
            .iter()
            .find(|path| path.is_selected())
            .map(|path| path.rtt())
    }

    pub async fn join(
        endpoint: &Endpoint,
        invite: &Invite,
        name: String,
        media: MediaState,
    ) -> Result<Self> {
        let connection = endpoint.connect(invite.server.clone(), ALPN).await?;
        let api = irpc::Client::<MeetProtocol>::boxed(irpc_iroh::IrohRemoteConnection::new(
            connection.clone(),
        ));
        let (updates, mut events) = api
            .bidi_streaming(
                JoinRoom {
                    room: invite.room,
                    name,
                    media,
                },
                1,
                1,
            )
            .await?;
        let event = events
            .recv()
            .await?
            .context("server disconnected during admission")?;

        match event {
            RoomEvent::Joined { generation, roster } => Ok(Self {
                connection,
                updates,
                events,
                generation,
                roster,
            }),
            RoomEvent::Rejected(reason) => {
                connection.close(0_u32.into(), b"rejected");

                Err(reason.into())
            }
            RoomEvent::Roster(_) => anyhow::bail!("server sent a roster before admission"),
        }
    }
}

impl Drop for RoomConnection {
    fn drop(&mut self) {
        self.connection.close(0_u32.into(), b"left");
    }
}

pub async fn connect_media(
    endpoint: &Endpoint,
    invite: &Invite,
    generation: &str,
    outgoing: &moq_net::origin::Producer,
    incoming: moq_net::origin::Producer,
) -> Result<(moq_net::Session, moq_net::Driver)> {
    let transport = iroh_moq::dial(endpoint, invite.server.clone()).await?;
    let pair = moq_net::Client::new()
        .with_path(crate::protocol::media_path(invite.room, generation))
        .with_publisher(outgoing)
        .with_subscriber(incoming)
        .connect(transport)
        .await?;

    Ok(pair)
}
