use axum::extract::ws::Message;
use axum::extract::{Path, Query, Request, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Json, Response};
use resource_island_server::dtos::{GameStateResponse, PlayerInfoResponse};
use resource_island_server::enums::ServerToPlayerMessage;
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
    let provided_header = request.headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok());
    let provided_query = request.uri().query().unwrap_or("");
    let provided_query = parse_params(provided_query);
    let provided_query = provided_query.get("token").map(|s| s.as_str());
    let provided_auth = {
        if provided_query.is_some() {
            provided_query
        }else if provided_header.is_some() {
            provided_header
        }else {
            None
        }
    };
    let token = {
        state.cfg.lock().server.token.clone()
    };
    if let Some(provided_auth) = provided_auth {
        trace!("Provided token: {}", provided_auth);
        trace!("Expected token: {}", token);
        if provided_auth != token {
            return Err(StatusCode::FORBIDDEN);
        }
    }else {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(request).await)
}

pub async fn root() -> &'static str {
    "You are all set!"
}
pub async fn get_game_state(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let state_guard = state.game_state.read();
    (StatusCode::OK, Json(GameStateResponse::from(&*state_guard)))
}
pub async fn get_player_info_with_path(State(state): State<Arc<AppState>>, Path(player_name): Path<String>) -> impl IntoResponse {
    let cfg = state.cfg.lock();
    drop(cfg);
    let guard = state.game_state.read();
    let player = match guard.players.get(player_name.as_str()){
        None => {
            return (StatusCode::NOT_FOUND, Json(PlayerInfoResponse::with_error()));
        },
        Some(p) => p,
    };
    (StatusCode::OK, Json(PlayerInfoResponse::from(player)))
}
pub async fn get_player_info_with_query(State(state): State<Arc<AppState>>, Query(args): Query<HashMap<String, String>>) -> impl IntoResponse {
    let player_name = match args.get("player") {
        Some(name) => name,
        None => {
            return (StatusCode::BAD_REQUEST, Json(PlayerInfoResponse::with_error()));
        },
    };
    let cfg = state.cfg.lock();
    drop(cfg);
    let guard = state.game_state.read();
    let player = match guard.players.get(player_name.as_str()){
        None => {
            return (StatusCode::NOT_FOUND, Json(PlayerInfoResponse::with_error()));
        },
        Some(p) => p,
    };
    (StatusCode::OK, Json(PlayerInfoResponse::from(player)))
}
pub async fn ws_handler(State(state): State<Arc<AppState>>, Path(player_name): Path<String>, ws: WebSocketUpgrade, Query(args): Query<HashMap<String, String>>) -> impl IntoResponse {
    ws.on_upgrade(move |mut socket| async move {
        let has_player = {
            let game_state = state.game_state.read();
            game_state.players.contains_key(player_name.as_str())
        };
        let (use_token, token_required) = {
            let cfg = state.cfg.lock();
            (cfg.server.use_token, cfg.server.token.clone())
        };
        if use_token {
            let token = match args.get("token") {
                Some(t) => t,
                None => {
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                },
            };
            if token.as_str() != token_required.as_str() {
                let _ = socket.send(Message::Close(None)).await;
                return;
            }
        }
        else if has_player {
            let _ = socket.send(Message::Close(None)).await;
            return;
        }
        {
            let mut game_state = state.game_state.write();
            let default_action_points = {
                let cfg = state.cfg.lock();
                cfg.game_rules.prepare.default_ap
            };
            let mut new_player = Player::new();
            new_player.action_points = default_action_points;
            game_state.players.insert(player_name.clone().leak(), new_player);
        }
        let spawn_player_name = player_name.clone();
        let spawn_state = state.clone();
        tokio::spawn(async move {
            loop {
                let maybe_msg = {
                    let game_guard = spawn_state.game_state.read();
                    let player = match game_guard.players.get(spawn_player_name.as_str()) {
                        Some(p) => p,
                        None => break,
                    };
                    match player.to_channel.receiver.recv() {
                        Ok(msg) => Some(msg),
                        Err(_) => None,
                    }
                };
                if maybe_msg.is_none() {
                    break;
                }
                let needs_to_send = maybe_msg.unwrap();
                match needs_to_send {
                    ServerToPlayerMessage::Broadcast { .. } => todo!(),
                }
            }
        });
    })
}