use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use iroh::EndpointId;
use moq_net::origin;
use tokio::{sync::watch, time::Instant};
use tokio_util::sync::CancellationToken;

use crate::protocol::{
    CAPACITY, JoinRoom, MediaState, Participant, Rejection, RoomId, media_origin,
};

const EMPTY_LIFETIME: Duration = Duration::from_secs(600);

#[derive(Clone, Default)]
pub struct Rooms(Arc<Mutex<HashMap<RoomId, Room>>>);

struct Room {
    members: Vec<Member>,
    changed: watch::Sender<Vec<Participant>>,
    empty_since: Option<Instant>,
    origin: origin::Producer,
}

#[derive(Debug)]
struct Member {
    peer: EndpointId,
    generation: String,
    participant: Participant,
    revoked: CancellationToken,
}

pub struct Membership {
    rooms: Rooms,
    room: RoomId,
    peer: EndpointId,
    pub generation: String,
    pub roster: watch::Receiver<Vec<Participant>>,
    pub revoked: CancellationToken,
}

pub struct MediaAccess {
    pub publish: origin::Producer,
    pub subscribe: origin::Producer,
    pub revoked: CancellationToken,
}

impl std::fmt::Debug for Rooms {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rooms").finish_non_exhaustive()
    }
}

impl Rooms {
    pub fn create(&self) -> RoomId {
        let mut rooms = self.0.lock().expect("room state poisoned");
        let room = loop {
            let room = rand::random();
            if !rooms.contains_key(&room) {
                break room;
            }
        };
        let (changed, _) = watch::channel(Vec::new());
        rooms.insert(
            room,
            Room {
                members: Vec::new(),
                changed,
                empty_since: Some(Instant::now()),
                origin: media_origin(),
            },
        );

        room
    }

    pub fn join(&self, peer: EndpointId, request: JoinRoom) -> Result<Membership, Rejection> {
        let name = request.name.trim();

        if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
            return Err(Rejection::InvalidName);
        }

        let mut rooms = self.0.lock().expect("room state poisoned");
        let room = rooms.get_mut(&request.room).ok_or(Rejection::Expired)?;

        if room.expired() {
            rooms.remove(&request.room);
            return Err(Rejection::Expired);
        }

        let previous = room.members.iter().position(|member| member.peer == peer);

        if previous.is_none() && room.members.len() == CAPACITY {
            return Err(Rejection::Full);
        }

        let generation = format!("{:032x}", rand::random::<u128>());
        let revoked = CancellationToken::new();
        let member = Member {
            peer,
            generation: generation.clone(),
            revoked: revoked.clone(),
            participant: Participant {
                id: peer.to_string(),
                name: name.to_owned(),
                broadcast: format!("{peer}/{generation}/camera.hang"),
                media: request.media,
            },
        };

        if let Some(index) = previous {
            room.members[index].revoked.cancel();
            room.members[index] = member;
        } else {
            room.members.push(member);
        }

        room.empty_since = None;
        room.notify();

        Ok(Membership {
            rooms: self.clone(),
            room: request.room,
            peer,
            generation,
            roster: room.changed.subscribe(),
            revoked,
        })
    }

    pub fn authorize(&self, peer: EndpointId, path: &str) -> Option<MediaAccess> {
        let (room, generation) = path.strip_prefix('/')?.split_once('/')?;
        let room = u128::from_str_radix(room, 16).ok()?.to_be_bytes();
        let rooms = self.0.lock().expect("room state poisoned");
        let room = rooms.get(&room)?;
        let member = room
            .members
            .iter()
            .find(|member| member.peer == peer && member.generation == generation)?;
        let prefix = format!("{peer}/{generation}/");
        let subscribe = room.origin.scope(&[moq_net::Path::from(prefix)])?;

        Some(MediaAccess {
            publish: room.origin.clone(),
            subscribe,
            revoked: member.revoked.clone(),
        })
    }

    pub fn expire(&self) {
        self.0
            .lock()
            .expect("room state poisoned")
            .retain(|_, room| !room.expired());
    }
}

impl Room {
    fn expired(&self) -> bool {
        self.empty_since
            .is_some_and(|since| since.elapsed() >= EMPTY_LIFETIME)
    }

    fn notify(&self) {
        self.changed.send_replace(
            self.members
                .iter()
                .map(|member| member.participant.clone())
                .collect(),
        );
    }
}

impl Membership {
    pub fn update(&self, media: MediaState) {
        let mut rooms = self.rooms.0.lock().expect("room state poisoned");
        let Some(room) = rooms.get_mut(&self.room) else {
            return;
        };
        let Some(member) = room
            .members
            .iter_mut()
            .find(|member| member.peer == self.peer && member.generation == self.generation)
        else {
            return;
        };

        if member.participant.media != media {
            member.participant.media = media;
            room.notify();
        }
    }
}

impl Drop for Membership {
    fn drop(&mut self) {
        self.revoked.cancel();
        let mut rooms = self.rooms.0.lock().expect("room state poisoned");
        let Some(room) = rooms.get_mut(&self.room) else {
            return;
        };
        let before = room.members.len();
        room.members
            .retain(|member| member.peer != self.peer || member.generation != self.generation);

        if room.members.len() == before {
            return;
        }

        if room.members.is_empty() {
            room.empty_since = Some(Instant::now());
        }

        room.notify();
    }
}
