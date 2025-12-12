use std::sync::Arc;
use parking_lot::{Mutex, RwLock};
use crate::config::GameCfg;
use crate::GameState;

pub fn game_main_loop(cfg: Arc<Mutex<GameCfg>>, game_state: Arc<RwLock<GameState>>) {
    let total_epochs = cfg.lock().game_rules.prepare.total_epochs;
    loop {
        if game_state.read().epoch > total_epochs {
            break;
        }
    }
}