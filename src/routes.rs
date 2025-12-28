use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use axum::extract::{Path, Query, Request, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Json, Response};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use resource_island_server::dtos::{GameStateResponse, PlayerInfoResponse};
use resource_island_server::{AppState, Player};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::trace;

fn parse_params(input: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in input.split('&') {
        if pair.trim().is_empty() {
            continue;
        }
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap().trim();
        let value = parts.next().unwrap_or("").trim();
        if !key.is_empty() {
            map.insert(key.to_string(), value.to_string());
        }
    }
    map
}
pub async fn auth_middleware(
    state: State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let provided_header = request
        .headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok());
    let provided_query = request.uri().query().unwrap_or("");
    let provided_query = parse_params(provided_query);
    let provided_query = provided_query.get("token").map(|s| s.as_str());
    let provided_auth = {
        if provided_query.is_some() {
            provided_query
        } else if provided_header.is_some() {
            provided_header
        } else {
            None
        }
    };
    let token = { state.cfg.lock().await.server.token.clone() };
    if let Some(provided_auth) = provided_auth {
        trace!("Provided token: {}", provided_auth);
        trace!("Expected token: {}", token);
        if provided_auth != token {
            return Err(StatusCode::FORBIDDEN);
        }
    } else {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(request).await)
}

pub async fn root() -> &'static str {
    "You are all set!"
}
pub async fn get_game_state(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let state_guard = state.game_state.read().await;
    (StatusCode::OK, Json(GameStateResponse::from(&*state_guard)))
}
pub async fn get_player_info_with_path(
    State(state): State<Arc<AppState>>,
    Path(player_name): Path<String>,
) -> impl IntoResponse {
    let cfg = state.cfg.lock();
    drop(cfg);
    let guard = state.game_state.read().await;
    let player = match guard.players.get(player_name.as_str()) {
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(PlayerInfoResponse::with_error()),
            );
        }
        Some(p) => p,
    };
    (StatusCode::OK, Json(PlayerInfoResponse::from(player)))
}
pub async fn get_player_info_with_query(
    State(state): State<Arc<AppState>>,
    Query(args): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let player_name = match args.get("player") {
        Some(name) => name,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(PlayerInfoResponse::with_error()),
            );
        }
    };
    let guard = state.game_state.read().await;
    let player = match guard.players.get(player_name.as_str()) {
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(PlayerInfoResponse::with_error()),
            );
        }
        Some(p) => p,
    };
    (StatusCode::OK, Json(PlayerInfoResponse::from(player)))
}
pub async fn ws_handler(
    State(state): State<Arc<AppState>>,
    Path(player_name): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let player_obj = {
        let cfg_temp = state.cfg.lock().await;
        Player::with_cfg(&*cfg_temp)
    };
    let register_player = {
        let mut state_temp = state.game_state.write().await;
        state_temp
            .register_player(player_name.clone(), player_obj)
            .await
    };
    if let Err(_) = register_player.clone() {
        (StatusCode::CONFLICT, "Player already exists").into_response()
    } else {
        ws.on_upgrade(move |mut socket| handler_on_upgrade(state, player_name, socket))
    }
}
async fn handler_on_upgrade(state: Arc<AppState>, player_name: String, socket: WebSocket) {
    let (writer, reader) = socket.split();
    tokio::spawn(handler_reader(state.clone(), player_name.clone(), reader));
    tokio::spawn(handler_writer(state, player_name, writer));
}
async fn handler_reader(
    state: Arc<AppState>,
    player_name: String,
    mut reader: SplitStream<WebSocket>,
) {
    while let Some(Ok(msg)) = reader.next().await {
        match msg {
            Message::Text(msg) => { /* TODO: 完成消息处理 */ }
            Message::Close(_) => {
                state
                    .game_state
                    .write()
                    .await
                    .unregister_player(player_name.clone())
                    .await
                    .unwrap_or(());
                break;
            }
            _ => {}
        }
    }
}
async fn handler_writer(
    state: Arc<AppState>,
    player_name: String,
    mut writer: SplitSink<WebSocket, Message>,
) {
    let receiver = {
        let game_state = state.game_state.read().await;
        let player = game_state.players.get(player_name.as_str()).unwrap();
        player.to_channel.receiver.clone()
    };

    while let Some(msg) = { receiver.lock().await.recv().await.clone() } {
        let send_result = writer
            .send(Message::Text(Utf8Bytes::from(
                serde_json::to_string(&msg).unwrap().as_str(),
            )))
            .await;
        if let Err(_) = send_result {
            break;
        }
    }
}
