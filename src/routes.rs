use axum::extract::ws::Message;
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use resource_island_server::dtos::{GameStateResponse, PlayerInfoResponse};
use resource_island_server::AppState;
use std::collections::HashMap;
use std::sync::Arc;

pub async fn root() -> &'static str {
    "You are all set!"
}
pub async fn get_game_state(State(state): State<Arc<AppState>>, Query(args): Query<HashMap<String, String>>) -> impl IntoResponse {
    if state.cfg.lock().server.query_use_token {
        let token = match args.get("token") {
            Some(t) => t,
            None => return (
                StatusCode::UNAUTHORIZED,
                Json(GameStateResponse::with_unauthorized())
            ),
        };
        if token.as_str() != state.cfg.lock().server.token.as_str() {
            return (
                StatusCode::FORBIDDEN,
                Json(GameStateResponse::with_unauthorized())
            );
        }
    }
    let state_guard = state.game_state.read();
    (StatusCode::OK, Json(GameStateResponse::from(&*state_guard)))
}
pub async fn get_player_info(State(state): State<Arc<AppState>>, Path(player_name): Path<String>, Query(args): Query<HashMap<String, String>>) -> impl IntoResponse {
    if state.cfg.lock().server.query_use_token {
        let token = match args.get("token") {
            Some(t) => t,
            None => return (
                StatusCode::UNAUTHORIZED,
                Json(PlayerInfoResponse::with_unauthorized())
            ),
        };
        if token.as_str() != state.cfg.lock().server.token.as_str() {
            return (
                StatusCode::FORBIDDEN,
                Json(PlayerInfoResponse::with_unauthorized())
            );
        }
    }
    let player = state.game_state.read();
    let player = player.players.get(player_name.as_str()).unwrap();
    (StatusCode::OK, Json(PlayerInfoResponse::from(player)))
}
pub async fn ws_handler(State(state): State<Arc<AppState>>, Path(player_name): Path<String>, ws: WebSocketUpgrade, Query(args): Query<HashMap<String, String>>) -> impl IntoResponse {
    ws.on_upgrade(|mut socket| async move {
        if state.cfg.lock().server.use_token {
            let token = match args.get("token") {
                Some(t) => t,
                None => {
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                },
            };
            if token.as_str() != state.cfg.lock().server.token.as_str() {
                let _ = socket.send(Message::Close(None)).await;
                return;
            }
        }
        // WebSocket handling logic goes here
    })
}