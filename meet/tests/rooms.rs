use iroh::SecretKey;
use iroh_tickets::Ticket;
use zero_meet::{
    protocol::{Invite, JoinRoom, MediaState, Rejection, media_path},
    rooms::Rooms,
};

fn peer() -> iroh::EndpointId {
    SecretKey::from_bytes(&rand::random()).public()
}

fn request(room: [u8; 16]) -> JoinRoom {
    JoinRoom {
        room,
        name: "Taylor".into(),
        media: MediaState::default(),
    }
}

#[tokio::test(start_paused = true)]
async fn expiry_resets_only_when_last_member_leaves() {
    let rooms = Rooms::default();
    let room = rooms.create();
    tokio::time::advance(std::time::Duration::from_secs(599)).await;
    let member = rooms.join(peer(), request(room)).unwrap();
    tokio::time::advance(std::time::Duration::from_secs(601)).await;
    let other = rooms.join(peer(), request(room)).unwrap();
    drop(member);
    rooms.expire();
    assert_eq!(other.roster.borrow().len(), 1);
    drop(other);
    tokio::time::advance(std::time::Duration::from_secs(600)).await;
    assert!(matches!(
        rooms.join(peer(), request(room)),
        Err(Rejection::Expired)
    ));

    let unused = rooms.create();
    tokio::time::advance(std::time::Duration::from_secs(600)).await;
    rooms.expire();
    assert!(matches!(
        rooms.join(peer(), request(unused)),
        Err(Rejection::Expired)
    ));
}

#[test]
fn replacements_revoke_old_media_without_stale_cleanup() {
    let rooms = Rooms::default();
    let room = rooms.create();
    let peer = peer();
    let old = rooms.join(peer, request(room)).unwrap();
    let access = rooms
        .authorize(peer, &media_path(room, &old.generation))
        .unwrap();
    let replacement = rooms.join(peer, request(room)).unwrap();
    assert!(access.revoked.is_cancelled());
    assert!(
        rooms
            .authorize(peer, &media_path(room, &old.generation))
            .is_none()
    );
    drop(old);
    assert_eq!(replacement.roster.borrow().len(), 1);
    assert!(
        rooms
            .authorize(peer, &media_path(room, &replacement.generation))
            .is_some()
    );
    drop(replacement);
    assert!(rooms.authorize(peer, &media_path(room, "wrong")).is_none());
}

#[tokio::test]
async fn concurrent_admission_never_exceeds_capacity() {
    let rooms = Rooms::default();
    let room = rooms.create();
    let mut jobs = tokio::task::JoinSet::new();

    for _ in 0..64 {
        let rooms = rooms.clone();
        jobs.spawn(async move { rooms.join(peer(), request(room)) });
    }

    let mut members = Vec::new();
    let mut full = 0;
    while let Some(result) = jobs.join_next().await {
        match result.unwrap() {
            Ok(member) => members.push(member),
            Err(Rejection::Full) => full += 1,
            Err(error) => panic!("unexpected rejection: {error}"),
        }
    }

    assert_eq!(members.len(), 32);
    assert_eq!(full, 32);
    assert_eq!(members[0].roster.borrow().len(), 32);
}

#[test]
fn invites_and_identity_have_one_valid_representation() {
    let invite = Invite {
        server: peer().into(),
        room: rand::random(),
    };
    let decoded = Invite::parse(&invite.link()).unwrap();
    assert_eq!(decoded.server, invite.server);
    assert_eq!(decoded.room, invite.room);
    let mut bytes = invite.encode_bytes();
    bytes.push(0);
    assert!(Invite::decode_bytes(&bytes).is_err());
    assert!(Invite::parse("zero-meet://join/not-a-ticket").is_err());

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity");
    let first = zero_meet::server::load_key(&path).unwrap();
    let second = zero_meet::server::load_key(&path).unwrap();
    assert_eq!(first.public(), second.public());
    std::fs::write(path.clone(), [0; 31]).unwrap();
    assert!(zero_meet::server::load_key(&path).is_err());
}

#[test]
fn shared_invites_omit_addresses_and_reject_other_formats() {
    let invite = Invite {
        server: iroh::EndpointAddr::from(peer()).with_ip_addr("127.0.0.1:7777".parse().unwrap()),
        room: rand::random(),
    };
    let link = invite.link();
    let decoded = Invite::parse(&link).unwrap();

    assert_eq!(decoded.server, invite.server.id.into());
    assert_eq!(decoded.room, invite.room);
    assert_eq!(link.len(), 92);
    assert_eq!(decoded.link(), link);

    assert_eq!(Invite::parse(&format!("  {link}\n")).unwrap().link(), link);

    for invalid in [
        format!("zero-meet://join/{}", decoded.encode_string()),
        decoded.encode_string(),
        format!("{link}/"),
        "zero-meet://".into(),
        "zero-meet://not-a-ticket".into(),
        format!("{link}/extra"),
        format!("{link}?extra"),
    ] {
        assert!(Invite::parse(&invalid).is_err(), "accepted {invalid}");
    }
}
