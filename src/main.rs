use std::sync::Arc;
use axum::middleware::from_fn_with_state;
use axum::routing::{any, get};
use tracing::{trace, info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use resource_island_server::GameState;
use crate::routes::{get_game_state, get_player_info_with_query, get_player_info_with_path, root, ws_handler};

mod routes;

#[tokio::main]
async fn main(){
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();
    trace!("正在创建状态对象");
    let cfg = resource_island_server::config::load_configuration("config.yaml").await.unwrap();
    let mut game_state = GameState::new();
    game_state.initialize(&cfg);
    game_state.players.insert("测试玩家", resource_island_server::Player::new());
    let state = Arc::new(resource_island_server::AppState::new(
        cfg,
        game_state
    ));
    trace!("正在创建路由");
    let app = axum::Router::new()
        .route("/", get(root))
        .route("/gamestate", get(get_game_state))
        .route("/playerinfo/{player_name}", get(get_player_info_with_path))
        .route("/ws/{player_name}", any(ws_handler))
        .route("/playerinfo", get(get_player_info_with_query))
        .route_layer(from_fn_with_state(state.clone(), routes::auth_middleware))
        .with_state(state.clone());
    let cfg = state.cfg.lock();
    let whole_address = format!("{}:{}", cfg.server.bind_host.clone(), cfg.server.bind_port.clone());
    drop(cfg);
    trace!("游戏相关线程开启中");
    tokio::spawn(async move {
        resource_island_server::game::game_main_loop(state);
    });
    trace!("正在创建监听器");
    let listener = tokio::net::TcpListener::bind(whole_address).await.unwrap();
    info!("正在开启Web路由");
    let server = axum::serve(listener, app.into_make_service());
    if let Err(err) = server.await {
        error!("{}", err);
    }
}
