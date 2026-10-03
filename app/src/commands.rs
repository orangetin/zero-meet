use crate::session::{AppState, Call, Event};
use tauri::{State, ipc::Channel};
use zero_meet::protocol::{Invite, MediaState};

#[tauri::command]
pub async fn create_room(state: State<'_, AppState>, server: String) -> Result<String, String> {
    let result = async {
        let address = server.trim().parse::<iroh::EndpointId>()?;
        let endpoint = state.endpoint().await?;
        let invite = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            zero_meet::client::create(endpoint, address.into()),
        )
        .await??;

        Ok::<_, anyhow::Error>(invite.link())
    }
    .await;

    result.map_err(|error| format!("Could not create a room: {error:#}"))
}

#[tauri::command]
pub fn inspect_invite(invite: String) -> Result<String, String> {
    Invite::parse(&invite)
        .map(|invite| invite.link())
        .map_err(|_| "This invitation is invalid. Paste the complete invitation link.".into())
}

#[tauri::command]
pub async fn join_room(
    state: State<'_, AppState>,
    invite: String,
    name: String,
    media: MediaState,
    events: Channel<Event>,
) -> Result<(), String> {
    let invite = Invite::parse(&invite).map_err(|error| error.to_string())?;
    let endpoint = state
        .endpoint()
        .await
        .map_err(|error| error.to_string())?
        .clone();
    let mut active = state.active.lock().await;

    if let Some(previous) = active.take() {
        previous.close().await;
    }

    *active = Some(Call::start(endpoint, invite, name, media, events));

    Ok(())
}

#[tauri::command]
pub async fn set_media(state: State<'_, AppState>, media: MediaState) -> Result<(), String> {
    if let Some(call) = state.active.lock().await.as_ref() {
        call.media.send_replace(media);
    }

    Ok(())
}

#[tauri::command]
pub async fn leave_room(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(call) = state.active.lock().await.take() {
        call.close().await;
    }

    Ok(())
}
