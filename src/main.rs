use std::sync::Arc;
use axum::routing::{any, get};
use tracing::{trace, info, warn, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use resource_island_server::GameState;
use crate::routes::{get_game_state, get_player_info, root, ws_handler};

mod routes;

#[tokio::main]
async fn main(){
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();
    trace!("正在创建状态对象");
    let cfg = resource_island_server::config::load_configuration("config.yaml").await.unwrap();
    let mut game_state = GameState::new();
    game_state.apply_configurations(&cfg);
    game_state.players.insert("测试玩家", resource_island_server::Player::new());
    let state = Arc::new(resource_island_server::AppState::new(
        cfg,
        game_state
    ));
    let whole_address = format!("{}:{}", state.cfg.lock().server.bind_host, state.cfg.lock().server.bind_port);
    trace!("正在创建路由");
    let app = axum::Router::new()
        .route("/", get(root))
        .route("/gamestate", get(get_game_state))
        .route("/playerinfo/{player_name}", get(get_player_info))
        .route("/ws/{player_name}", any(ws_handler))
        .with_state(state);
    trace!("正在创建监听器");
    let listener = tokio::net::TcpListener::bind(whole_address).await.unwrap();
    info!("正在开启Web路由");
    if let Err(obj) = axum::serve(listener, app).await {
        error!("{}", obj);
    }
}
