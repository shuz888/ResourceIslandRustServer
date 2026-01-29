use crate::enums::{ServerBroadcastMessage, ServerToPlayerMessage};
use crate::structs::{AppState, GameStateResponse};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::time::sleep;
use tracing::info;

pub async fn game_main_loop(app_state: Arc<AppState>) {
    let (total_epochs, required_players) = {
        let config = app_state.cfg.clone();
        (
            config.game_rules.prepare.total_epochs,
            config.server.player_numbers,
        )
    };
    loop {
        {
            let game_state = app_state.game_state.read().await;
            if game_state.players.len() as u32 >= required_players {
                break;
            }
        }
    }
    info!("游戏现在开始");
    {
        let game_state = app_state.game_state.read().await;
        game_state
            .broadcast(ServerBroadcastMessage::GameStart {})
            .await;
    }
    {
        let mut game_state = app_state.game_state.write().await;
        game_state.started = true;
    }
    let app_state_heartbeat = app_state.clone();
    tokio::spawn(async move {
        let heartbeat_interval = app_state_heartbeat.cfg.server.heart_beat_interval;
        let dur = Duration::from_secs(heartbeat_interval as u64);
        loop {
            {
                let state = app_state_heartbeat.game_state.read().await;
                let resp = GameStateResponse::from(&*state);
                state
                    .broadcast(ServerBroadcastMessage::HeartBeat {
                        state: resp,
                        interval: heartbeat_interval,
                    })
                    .await;
            }
            sleep(dur).await;
        }
    });
    loop {
        {
            if app_state.game_state.read().await.epoch > total_epochs {
                return;
            }
        }
        let (cur_phase, cur_epoch) = {
            let guard = app_state.game_state.read().await;
            (guard.phase, guard.epoch)
        };
        if cur_phase == 1 {
            if !app_state.cfg.game_rules.investment.enable {
                let mut game_state = app_state.game_state.write().await;
                game_state.increase_phase().await;
                drop(game_state);
                continue;
            }
            {
                let game_state = app_state.game_state.read().await;
                game_state
                    .broadcast(ServerBroadcastMessage::PhaseChanged {
                        phase: cur_phase,
                        epoch: cur_epoch,
                    })
                    .await;
                for pl in game_state.players.keys() {
                    game_state
                        .send_to(
                            pl,
                            ServerToPlayerMessage::DataRequired {
                                phase: cur_phase,
                                epoch: cur_epoch,
                            },
                        )
                        .await;
                }
            }
            loop {
                let receivers: Vec<_> = {
                    let game_state = app_state.game_state.read().await;
                    game_state
                        .players
                        .iter()
                        .map(|(name, player)| {
                            (name.to_string(), player.from_channel.receiver.clone())
                        })
                        .collect()
                };
                for (_player_name, receiver) in receivers {
                    let msg = receiver.lock().await.try_recv();
                    if msg.is_err() {
                        let err = msg.err().unwrap();
                        match err {
                            TryRecvError::Empty => continue,
                            TryRecvError::Disconnected => {
                                return;
                            }
                        }
                    }
                    // TODO: 处理接收到的消息
                }
            }
        } else if cur_phase == 2 {
        } else if cur_phase == 3 {
        } else if cur_phase == 4 {
        }
        {
            let mut game_state = app_state.game_state.write().await;
            game_state.increase_phase().await;
        }
    }
}
