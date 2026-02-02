use super::structs::{AppState, GameStateResponse, Player, PlayerInfoResponse};
use crate::enums::{PlayerToServerMessage, ServerToPlayerMessage};
use axum::extract::ws::{Message, Utf8Bytes, WebSocket};
use axum::extract::{Path, Request, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info};
use uuid::Uuid;

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
    let (token, use_token) = {
        let cfg = state.cfg.clone();
        (cfg.server.token.clone(), cfg.server.use_token)
    };
    if token == "__set_token_here__" || !use_token {
        return Ok(next.run(request).await);
    }
    if let Some(provided_auth) = provided_auth {
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
pub async fn ws_handler(
    State(state): State<Arc<AppState>>,
    Path(player_name): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let player_obj = {
        let cfg_temp = state.cfg.clone();
        Player::with_cfg(player_name.clone().leak(), &*cfg_temp)
    };
    let uuid = {
        let mut state_temp = state.game_state.write().await;
        state_temp.register_player(player_obj).await
    };
    ws.on_upgrade(move |socket| handler_on_upgrade(state, uuid, socket))
}
async fn handler_on_upgrade(state: Arc<AppState>, uuid: Uuid, socket: WebSocket) {
    let (mut writer, reader) = socket.split();
    info!("有玩家加入，分配UUID：{:?}", uuid);
    let msg = ServerToPlayerMessage::UuidNotice { uuid };
    let send_result = writer
        .send(Message::Text(Utf8Bytes::from(
            serde_json::to_string(&msg).unwrap().as_str(),
        )))
        .await;
    if send_result.is_err() {
        return;
    }
    let reader = tokio::spawn(handler_reader(state.clone(), uuid.clone(), reader));
    let writer = tokio::spawn(handler_writer(state.clone(), uuid.clone(), writer));
    tokio::select! {
        _ = reader => {}
        _ = writer => {}
    }
    state
        .game_state
        .write()
        .await
        .unregister_player(&uuid)
        .await;
}
async fn handler_reader(state: Arc<AppState>, uuid: Uuid, mut reader: SplitStream<WebSocket>) {
    while let Some(Ok(msg)) = reader.next().await {
        debug!("接收到消息");
        match msg {
            Message::Text(msg) => {
                let str = msg.to_string();
                let msg = serde_json::from_str::<PlayerToServerMessage>(str.as_str());
                match msg {
                    Ok(msg) => {
                        if let PlayerToServerMessage::RequestGameState {} = msg.clone() {
                            let sender = {
                                let game_state = state.game_state.read().await;
                                game_state
                                    .players
                                    .get(&uuid.clone())
                                    .unwrap()
                                    .to_channel
                                    .sender
                                    .clone()
                            };
                            let resp = {
                                let game_state = state.game_state.read().await;
                                GameStateResponse::from(&*game_state)
                            };
                            let resp = ServerToPlayerMessage::GameStateResponse { state: resp };
                            if sender.send(resp).await.is_err() {
                                break;
                            }
                            continue;
                        } else if let PlayerToServerMessage::RequestPlayerInfo {
                            uuid: requested_uuid,
                        } = msg.clone()
                        {
                            let sender = {
                                let game_state = state.game_state.read().await;
                                game_state
                                    .players
                                    .get(&uuid.clone())
                                    .unwrap()
                                    .to_channel
                                    .sender
                                    .clone()
                            };
                            let resp = {
                                let game_state = state.game_state.read().await;
                                let player = game_state.players.get(&requested_uuid);
                                if player.is_none() {
                                    continue;
                                }
                                PlayerInfoResponse::from(&*player.unwrap())
                            };
                            let resp = ServerToPlayerMessage::PlayerInfoResponse {
                                uuid: uuid.clone(),
                                player: resp,
                            };
                            if sender.send(resp).await.is_err() {
                                break;
                            }
                        }
                        let sender = {
                            let game_state = state.game_state.read().await;
                            game_state
                                .players
                                .get(&uuid.clone())
                                .unwrap()
                                .from_channel
                                .sender
                                .clone()
                        };
                        let snd = sender.send(msg).await;
                        if snd.is_err() {
                            break;
                        }
                    }
                    Err(obj) => {
                        error!("json反序列化出错：{:?}", obj);
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}
async fn handler_writer(
    state: Arc<AppState>,
    uuid: Uuid,
    mut writer: SplitSink<WebSocket, Message>,
) {
    let receiver = {
        let game_state = state.game_state.read().await;
        let player = game_state.players.get(&uuid).unwrap();
        player.to_channel.receiver.clone()
    };

    loop {
        let msg = {
            let mut rx = receiver.lock().await;
            rx.recv().await
        };

        if let Some(msg) = msg {
            debug!("准备发送消息");
            let send_result = writer
                .send(Message::Text(Utf8Bytes::from(
                    serde_json::to_string(&msg).unwrap().as_str(),
                )))
                .await;
            if send_result.is_err() {
                break;
            }
        } else {
            break;
        }
    }
}
