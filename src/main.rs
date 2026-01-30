use axum::middleware::from_fn_with_state;
use axum::routing::{any, get};
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;
use rand_seeder::SipHasher;
use rand_seeder::rand_core::RngCore;
use resource_island_server::config::GameCfg;
use resource_island_server::routes;
use resource_island_server::routes::{root, ws_handler};
use resource_island_server::structs::{AppState, GameState};
use std::sync::Arc;
use tracing::{debug, error, info, trace};
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    let env_filter = EnvFilter::try_from_env("RSILS_LOG")
        .unwrap_or_else(|_| EnvFilter::new("debug,tokio_tungstenite=info"));
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(env_filter))
        .init();
    info!("游戏正加载中，请稍后");
    // 加载配置
    trace!("加载配置");
    let cfg = GameCfg::load_from("config.yaml".into());
    if cfg.is_err() {
        error!("加载配置失败，正在使用默认配置");
    }
    let cfg = cfg.unwrap_or(Default::default());
    // 根据配置文件指定的种子生成随机数生成器
    trace!("根据配置文件指定的种子生成随机数生成器");
    let hasher = SipHasher::from(cfg.server.seed.clone());
    let mut hasher_rng = hasher.into_rng();
    let mut seed: <ChaCha20Rng as SeedableRng>::Seed = Default::default();
    hasher_rng.fill_bytes(&mut seed);
    let mut rng = ChaCha20Rng::from_seed(seed);
    drop(hasher_rng);
    if cfg.server.seed.clone() == "__set_seed_here__" {
        rng = ChaCha20Rng::from_os_rng();
    }
    debug!("种子为：{:?}", rng.get_seed());
    // 游戏状态对象创建
    trace!("游戏状态对象创建");
    let mut game_state = GameState::new();
    game_state.initialize(&cfg, &mut rng).await;
    let state = Arc::new(AppState::new(cfg, game_state, rng));
    // 创建路由
    trace!("创建路由");
    let app = axum::Router::new()
        .route("/", get(root))
        .route("/ws/{player_name}", any(ws_handler))
        .route_layer(from_fn_with_state(state.clone(), routes::auth_middleware))
        .with_state(state.clone());
    // 完整地址生成
    trace!("完整地址生成");
    let cfg = state.clone().cfg.clone();
    let whole_address = format!(
        "{}:{}",
        cfg.server.bind_host.clone(),
        cfg.server.bind_port.clone()
    );
    // 开启游戏主线程
    trace!("开启游戏主线程");
    tokio::spawn(resource_island_server::game::game_main_loop(state));
    // 创建服务
    trace!("创建服务");
    let listener = tokio::net::TcpListener::bind(whole_address).await.unwrap();
    let server = axum::serve(listener, app.into_make_service());
    info!("正在开启Web路由");
    if let Err(err) = server.await {
        error!("游戏进程出错：{:?}", err);
    }
}
