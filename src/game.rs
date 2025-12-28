use crate::AppState;
use crate::enums::ServerBroadcastMessage;
use std::sync::Arc;

pub async fn game_main_loop(app_state: Arc<AppState>) {
    let (total_epochs, required_players) = {
        let config = app_state.cfg.lock().await;
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
    {
        let game_state = app_state.game_state.read().await;
        game_state
            .broadcast(ServerBroadcastMessage::GameStart)
            .await;
    }
    {
        let mut game_state = app_state.game_state.write().await;
        game_state.started = true;
    }
    loop {
        {
            if app_state.game_state.read().await.epoch > total_epochs {
                break;
            }
        }
        /* TODO: 完成游戏逻辑 */
        {
            let mut game_state = app_state.game_state.write().await;
            game_state.phase += 1;
            if game_state.phase == 5 {
                game_state.epoch += 1;
                game_state.phase = 0;
            }
        }
    }
}
