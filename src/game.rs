use std::sync::Arc;
use crate::AppState;
use crate::enums::ServerBroadcastMessage;

pub fn game_main_loop(app_state: Arc<AppState>) {
    let (total_epochs, required_players) = {
        let config = app_state.cfg.lock();
        (config.game_rules.prepare.total_epochs, config.server.player_numbers)
    };
    loop {
        {
            let game_state = app_state.game_state.read();
            if game_state.players.len() as u32 >= required_players {
                break;
            }
        }
    }
    {
        let channels = &app_state.channels;
        match channels.server_broadcast.sender.clone().send(ServerBroadcastMessage::GameStart) {
            Ok(_) => (),
            Err(_) => { return; }
        }
    }
    loop {
        {
            if app_state.game_state.read().epoch > total_epochs {
                break;
            }
        }
    }
}