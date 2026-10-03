use iroh::EndpointAddr;
use iroh_tickets::{ParseError, Ticket};
use irpc::{
    channel::{mpsc, oneshot},
    rpc_requests,
};
use serde::{Deserialize, Serialize};

pub const ALPN: &[u8] = b"zero-meet/control/1";
pub const CAPACITY: usize = 32;
pub type RoomId = [u8; 16];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invite {
    pub server: EndpointAddr,
    pub room: RoomId,
}

impl Ticket for Invite {
    const KIND: &'static str = "meet";

    fn encode_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(&(1_u8, self)).expect("invite serializes")
    }

    fn decode_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        let ((version, invite), remainder): ((u8, Self), _) = postcard::take_from_bytes(bytes)?;

        if version != 1 || !remainder.is_empty() {
            return Err(ParseError::verification_failed("unsupported invite"));
        }

        Ok(invite)
    }
}

impl Invite {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        let value = value.trim();
        anyhow::ensure!(value.len() <= 4096, "Invitation is too long");

        let payload = value
            .strip_prefix("zero-meet://")
            .ok_or_else(|| anyhow::anyhow!("Invitation must start with zero-meet://"))?;
        let ticket = format!("{}{payload}", Self::KIND);

        Ok(Self::decode_string(&ticket)?)
    }

    pub fn link(&self) -> String {
        // The N0 preset discovers the server's current addresses from its identity.
        // Keep changing relay and network addresses out of shared invitations.
        let invite = Self {
            server: self.server.id.into(),
            room: self.room,
        };

        let ticket = invite.encode_string();
        let payload = &ticket[Self::KIND.len()..];

        format!("zero-meet://{payload}")
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaState {
    pub microphone: bool,
    pub camera: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Participant {
    pub id: String,
    pub name: String,
    pub broadcast: String,
    pub media: MediaState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rejection {
    Expired,
    Full,
    InvalidName,
}

impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Expired => "This room has expired. Ask for a new invitation.",
            Self::Full => "This room has reached its 32-person limit.",
            Self::InvalidName => "Enter a name between 1 and 64 characters.",
        })
    }
}

impl std::error::Error for Rejection {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoomEvent {
    Joined {
        generation: String,
        roster: Vec<Participant>,
    },
    Roster(Vec<Participant>),
    Rejected(Rejection),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRoom;

#[derive(Debug, Serialize, Deserialize)]
pub struct JoinRoom {
    pub room: RoomId,
    pub name: String,
    pub media: MediaState,
}

#[rpc_requests(message = MeetMessage)]
#[derive(Debug, Serialize, Deserialize)]
pub enum MeetProtocol {
    #[rpc(tx = oneshot::Sender<RoomId>)]
    Create(CreateRoom),
    #[rpc(tx = mpsc::Sender<RoomEvent>, rx = mpsc::Receiver<MediaState>)]
    Join(JoinRoom),
}

pub fn media_path(room: RoomId, generation: &str) -> String {
    format!("/{:032x}/{generation}", u128::from_be_bytes(room))
}

pub fn media_origin() -> moq_net::origin::Producer {
    moq_net::origin::Info::new(moq_net::Origin::random())
        .with_pool(moq_net::cache::Pool::new(16 * 1024 * 1024))
        .with_cache_duration(std::time::Duration::from_secs(1))
        .with_linger(std::time::Duration::ZERO)
        .produce()
}
