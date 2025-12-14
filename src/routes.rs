use axum::extract::ws::Message;
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use resource_island_server::dtos::{GameStateResponse, PlayerInfoResponse};
use resource_island_server::AppState;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

pub async fn root() -> &'static str {
    "You are all set!"
}
pub async fn get_game_state(State(state): State<Arc<AppState>>, Query(args): Query<HashMap<String, String>>) -> impl IntoResponse {
    let cfg = state.cfg.lock();
    let query_use_token = cfg.server.query_use_token.clone();
    let token_required = cfg.server.token.clone();
    drop(cfg);
    if query_use_token {
        let token = match args.get("token") {
            Some(t) => t,
            None => return (
                StatusCode::UNAUTHORIZED,
                Json(GameStateResponse::with_error())
            ),
        };
        if token.as_str() != token_required.as_str() {
            return (
                StatusCode::FORBIDDEN,
                Json(GameStateResponse::with_error())
            );
        }
    }
    let state_guard = state.game_state.read();
    let state_guard = state_guard.deref();
    (StatusCode::OK, Json(GameStateResponse::from(state_guard)))
}
pub async fn get_player_info_with_path(State(state): State<Arc<AppState>>, Path(player_name): Path<String>, Query(args): Query<HashMap<String, String>>) -> impl IntoResponse {
    let cfg = state.cfg.lock();
    let query_use_token = cfg.server.query_use_token.clone();
    let token_required = cfg.server.token.clone();
    drop(cfg);
    if query_use_token {
        let token = match args.get("token") {
            Some(t) => t,
            None => return (
                StatusCode::UNAUTHORIZED,
                Json(PlayerInfoResponse::with_error())
            ),
        };
        if token.as_str() != token_required.as_str() {
            return (
                StatusCode::FORBIDDEN,
                Json(PlayerInfoResponse::with_error())
            );
        }
    }
    let player = state.game_state.read();
    let player = match player.players.get(player_name.as_str()){
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
    let query_use_token = cfg.server.query_use_token.clone();
    let token_required = cfg.server.token.clone();
    drop(cfg);
    if query_use_token {
        let token = match args.get("token") {
            Some(t) => t,
            None => return (
                StatusCode::UNAUTHORIZED,
                Json(PlayerInfoResponse::with_error())
            ),
        };
        if token.as_str() != token_required.as_str() {
            return (
                StatusCode::FORBIDDEN,
                Json(PlayerInfoResponse::with_error())
            );
        }
    }
    let player = state.game_state.read();
    let player = match player.players.get(player_name.as_str()){
        None => {
            return (StatusCode::NOT_FOUND, Json(PlayerInfoResponse::with_error()));
        },
        Some(p) => p,
    };
    (StatusCode::OK, Json(PlayerInfoResponse::from(player)))
}
pub async fn ws_handler(State(state): State<Arc<AppState>>, Path(player_name): Path<String>, ws: WebSocketUpgrade, Query(args): Query<HashMap<String, String>>) -> impl IntoResponse {
    ws.on_upgrade(|mut socket| async move {
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
    })
}