pub mod game {
    use crate::enums::{
        InvestmentAction, InvestmentError, Items, PlayerToServerMessage, ServerBroadcastMessage,
        ServerToPlayerMessage,
    };
    use crate::structs::{AppState, GameStateResponse};
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::mpsc::error::TryRecvError;
    use tokio::time::sleep;
    use tracing::info;
    use uuid::Uuid;

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
                /* TODO: 添加玩家机器处理 */
                let game_state = app_state.game_state.read().await;
                let mut investment_unfinished: HashSet<Uuid> =
                    game_state.players.keys().map(|pl| pl.clone()).collect();
                let mut counts: HashMap<Uuid, HashMap<&'static str, u32>> = game_state
                    .players
                    .keys()
                    .map(|pl| {
                        let actions: Vec<&'static str> = vec![
                            "exchange",
                            "explore",
                            "build",
                            "mine",
                            "ore",
                            "storemoney",
                            "pick",
                        ];
                        let action_map: HashMap<&'static str, u32> =
                            actions.into_iter().map(|s| (s, 0)).collect();
                        (pl.clone(), action_map)
                    })
                    .collect();
                drop(game_state);
                loop {
                    if investment_unfinished.is_empty() {
                        break;
                    }
                    let senders_and_receivers: Vec<_> = {
                        let game_state = app_state.game_state.read().await;
                        game_state
                            .players
                            .iter()
                            .map(|(uuid, player)| {
                                (
                                    uuid.clone(),
                                    (
                                        player.from_channel.sender.clone(),
                                        player.from_channel.receiver.clone(),
                                    ),
                                )
                            })
                            .collect()
                    };
                    for (uuid, (sender, receiver)) in senders_and_receivers {
                        if !investment_unfinished.contains(&uuid) {
                            continue;
                        }
                        let msg = receiver.lock().await.try_recv();
                        if msg.is_err() {
                            let err = msg.err().unwrap();
                            match err {
                                TryRecvError::Empty => continue,
                                TryRecvError::Disconnected => {
                                    let mut game_state = app_state.game_state.write().await;
                                    game_state.unregister_player(&uuid).await;
                                }
                            }
                            continue;
                        }
                        let msg = msg.unwrap();
                        match msg {
                            PlayerToServerMessage::SendInvestment { action } => match action {
                                InvestmentAction::Explore {} => {
                                    let items_per_ap = app_state
                                        .cfg
                                        .game_rules
                                        .investment
                                        .action_cfg
                                        .explore
                                        .items_per_ap;
                                    let explore_limits = app_state
                                        .cfg
                                        .game_rules
                                        .investment
                                        .action_cfg
                                        .explore
                                        .explore_limits;
                                    let now_count =
                                        counts.get(&uuid).unwrap().get("explore").unwrap().clone();
                                    let game_state = app_state.game_state.read().await;
                                    if now_count > explore_limits && explore_limits != 0 {
                                        game_state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::LimitsExceeded {
                                                        limit: explore_limits,
                                                    }),
                                                },
                                            )
                                            .await;
                                        continue;
                                    } else {
                                        counts
                                            .get_mut(&uuid)
                                            .unwrap()
                                            .insert("explore", now_count + 1);
                                    }
                                    if game_state.players.get(&uuid).unwrap().action_points < 1 {
                                        game_state.send_to(&uuid, ServerToPlayerMessage::InvestmentResult {
                                                action,
                                                error: true,
                                                reason: Some(InvestmentError::ActionPointsDoesNotEnough { need: 1 })
                                            }).await;
                                        continue;
                                    }
                                    drop(game_state);
                                    let mut game_state = app_state.game_state.write().await;
                                    game_state.players.get_mut(&uuid).unwrap().action_points -= 1;
                                    let mut cards: Vec<_> = game_state
                                        .current_deck
                                        .drain(..items_per_ap as usize)
                                        .collect();
                                    game_state.market.append(&mut cards);
                                    drop(cards);
                                    let game_state = game_state.downgrade();
                                    game_state
                                        .send_to(
                                            &uuid,
                                            ServerToPlayerMessage::InvestmentResult {
                                                action,
                                                error: false,
                                                reason: None,
                                            },
                                        )
                                        .await;
                                }
                                InvestmentAction::Exchange {} => {
                                    let ap_per_food = app_state
                                        .cfg
                                        .game_rules
                                        .investment
                                        .action_cfg
                                        .exchange
                                        .ap_per_food;
                                    let exchange_limits = app_state
                                        .cfg
                                        .game_rules
                                        .investment
                                        .action_cfg
                                        .exchange
                                        .exchange_limits;
                                    let now_limits =
                                        counts.get(&uuid).unwrap().get("exchange").unwrap().clone();
                                    if now_limits > exchange_limits && exchange_limits != 0 {
                                        let game_state = app_state.game_state.read().await;
                                        game_state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(InvestmentError::LimitsExceeded {
                                                        limit: exchange_limits,
                                                    }),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    let mut game_state = app_state.game_state.write().await;
                                    let player_mut = game_state.players.get_mut(&uuid).unwrap();
                                    if player_mut.resources.get(&Items::Food).unwrap() < &1 {
                                        game_state
                                            .send_to(
                                                &uuid,
                                                ServerToPlayerMessage::InvestmentResult {
                                                    action,
                                                    error: true,
                                                    reason: Some(
                                                        InvestmentError::FoodDoesNotEnough {
                                                            need: 1,
                                                        },
                                                    ),
                                                },
                                            )
                                            .await;
                                        continue;
                                    }
                                    let old =
                                        player_mut.resources.get(&Items::Food).unwrap().clone();
                                    player_mut.resources.insert(Items::Food, old - 1);
                                    player_mut.action_points += ap_per_food;
                                    game_state
                                        .send_to(
                                            &uuid,
                                            ServerToPlayerMessage::InvestmentResult {
                                                action,
                                                error: false,
                                                reason: None,
                                            },
                                        )
                                        .await;
                                }
                                InvestmentAction::Build { .. } => {
                                    todo!()
                                }
                                InvestmentAction::Ore { .. } => {
                                    todo!()
                                }
                                InvestmentAction::Pick { .. } => {
                                    todo!()
                                }
                                InvestmentAction::Mine { .. } => {
                                    todo!()
                                }
                                InvestmentAction::StoreMoney { .. } => {
                                    todo!()
                                }
                                InvestmentAction::End {} => {
                                    investment_unfinished.remove(&uuid);
                                }
                            },
                            _ => {
                                if sender.send(msg).await.is_err() {
                                    return;
                                };
                            }
                        }
                    }
                }
            } else if cur_phase == 2 {
            } else if cur_phase == 3 {
            } else if cur_phase == 4 {
            }
            let mut game_state = app_state.game_state.write().await;
            game_state.increase_phase().await;
        }
    }
}
pub mod config {
    use crate::enums::Items;
    use crate::structs::Player;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::fs::File;
    #[macro_export]
    macro_rules! impl_default_with {
    ($type_name:ident) => {
        impl Default for $type_name {
            fn default() -> Self {
                $type_name::with_defaults()
            }
        }
    };
    ($type_name:ident<$($generic:ident),*>) => {
        impl<$($generic),*> Default for $type_name<$($generic),*>
        where
            $($generic: Default),*
        {
            fn default() -> Self {
                $type_name::with_defaults()
            }
        }
    };
    ($type_name:ident<$($generic:ident),*> where $($bound:tt)*) => {
        impl<$($generic),*> Default for $type_name<$($generic),*>
        where
            $($bound)*
        {
            fn default() -> Self {
                $type_name::with_defaults()
            }
        }
    };
}

    #[derive(Serialize, Deserialize, Debug)]
    pub struct GameCfg {
        pub server: ServerCfg,
        pub game_rules: GameRules,
    }
    impl GameCfg {
        fn with_defaults() -> GameCfg {
            GameCfg {
                server: Default::default(),
                game_rules: Default::default(),
            }
        }
        pub fn load_from(file_name: String) -> anyhow::Result<Self> {
            if std::path::Path::new(&file_name).exists() == false {
                let file = File::create(&file_name)?;
                serde_yaml::to_writer::<File, GameCfg>(file, &Default::default())?;
            }
            let file = File::open(&file_name)?;
            let cfg = serde_yaml::from_reader(file);
            Ok(cfg?)
        }
    }
    impl_default_with!(GameCfg);
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ServerCfg {
        pub player_numbers: u32,
        pub use_token: bool,
        pub token: String,
        pub bind_host: String,
        pub bind_port: u32,
        pub seed: String,
        pub heart_beat_interval: u32,
    }
    impl ServerCfg {
        fn with_defaults() -> ServerCfg {
            ServerCfg {
                player_numbers: 4,
                use_token: false,
                token: "__set_the_token_here__".into(),
                bind_host: "0.0.0.0".into(),
                bind_port: 8080,
                seed: "__set_seed_here__".into(),
                heart_beat_interval: 10,
            }
        }
    }
    impl_default_with!(ServerCfg);
    #[derive(Serialize, Deserialize, Debug)]
    pub struct GameRules {
        pub prepare: PrepareCfg,
        pub resource_values_default: ResourceValuesDefault,
        pub investment: InvestmentCfg,
    }
    impl GameRules {
        fn with_defaults() -> GameRules {
            GameRules {
                prepare: Default::default(),
                resource_values_default: Default::default(),
                investment: Default::default(),
            }
        }
    }
    impl_default_with!(GameRules);
    #[derive(Serialize, Deserialize, Debug)]
    pub struct PrepareCfg {
        pub total_epochs: u32,
        pub draw_cards: u32,
        pub defaults_give_player: DefaultsGivePlayerCfg,
        pub deck: DeckCfg,
    }
    impl PrepareCfg {
        fn with_defaults() -> PrepareCfg {
            PrepareCfg {
                total_epochs: 10,
                draw_cards: 10,
                defaults_give_player: Default::default(),
                deck: Default::default(),
            }
        }
    }
    impl_default_with!(PrepareCfg);
    #[derive(Serialize, Deserialize, Debug)]
    pub struct DefaultsGivePlayerCfg {
        pub ap: u32,
        pub diamond: u32,
        pub gold: u32,
        pub wood: u32,
        pub ore: u32,
        pub food: u32,
        pub iron: u32,
    }
    impl DefaultsGivePlayerCfg {
        fn with_defaults() -> DefaultsGivePlayerCfg {
            DefaultsGivePlayerCfg {
                ap: 5,
                diamond: 0,
                gold: 0,
                wood: 0,
                ore: 0,
                food: 5,
                iron: 0,
            }
        }
        pub fn apply_to_player(&self, player: &mut Player) {
            player.action_points = self.ap;
            player.resources.insert(Items::Gold, self.gold);
            player.resources.insert(Items::Ore, self.ore);
            player.resources.insert(Items::Diamond, self.diamond);
            player.resources.insert(Items::Wood, self.wood);
            player.resources.insert(Items::Iron, self.iron);
            player.resources.insert(Items::Food, self.food);
        }
    }
    impl_default_with!(DefaultsGivePlayerCfg);
    #[derive(Serialize, Deserialize, Debug)]
    pub struct DeckCfg {
        pub diamond: u32,
        pub gold: u32,
        pub wood: u32,
        pub ore: u32,
        pub food: u32,
        pub iron: u32,
    }
    impl DeckCfg {
        fn with_defaults() -> DeckCfg {
            DeckCfg {
                diamond: 50,
                gold: 80,
                wood: 100,
                ore: 100,
                food: 100,
                iron: 100,
            }
        }
    }
    impl Into<HashMap<Items, u32>> for &DeckCfg {
        fn into(self) -> HashMap<Items, u32> {
            let mut res = HashMap::new();
            res.insert(Items::Diamond, self.diamond);
            res.insert(Items::Gold, self.gold);
            res.insert(Items::Wood, self.wood);
            res.insert(Items::Ore, self.ore);
            res.insert(Items::Food, self.food);
            res.insert(Items::Iron, self.iron);
            res
        }
    }
    impl_default_with!(DeckCfg);
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ResourceValuesDefault {
        pub diamond: u32,
        pub gold: u32,
        pub wood: u32,
        pub ore: u32,
        pub food: u32,
        pub iron: u32,
    }
    impl ResourceValuesDefault {
        fn with_defaults() -> ResourceValuesDefault {
            ResourceValuesDefault {
                diamond: 8,
                gold: 6,
                wood: 2,
                ore: 3,
                food: 1,
                iron: 2,
            }
        }
    }
    impl Into<HashMap<Items, u32>> for &ResourceValuesDefault {
        fn into(self) -> HashMap<Items, u32> {
            let mut res = HashMap::new();
            res.insert(Items::Diamond, self.diamond);
            res.insert(Items::Gold, self.gold);
            res.insert(Items::Wood, self.wood);
            res.insert(Items::Ore, self.ore);
            res.insert(Items::Food, self.food);
            res.insert(Items::Iron, self.iron);
            res
        }
    }
    impl_default_with!(ResourceValuesDefault);
    #[derive(Serialize, Deserialize, Debug)]
    pub struct InvestmentCfg {
        pub enable: bool,
        pub action_cfg: InvestmentActionCfg,
    }
    impl InvestmentCfg {
        fn with_defaults() -> InvestmentCfg {
            InvestmentCfg {
                enable: true,
                action_cfg: Default::default(),
            }
        }
    }
    impl_default_with!(InvestmentCfg);
    #[derive(Serialize, Deserialize, Debug)]
    pub struct InvestmentActionCfg {
        pub explore: ExploreSettings,
        pub exchange: ExchangeSettings,
    }
    impl InvestmentActionCfg {
        fn with_defaults() -> InvestmentActionCfg {
            InvestmentActionCfg {
                explore: Default::default(),
                exchange: Default::default(),
            }
        }
    }
    impl_default_with!(InvestmentActionCfg);
    #[derive(Serialize, Deserialize, Debug)]
    pub struct BuildCfg {
        pub enabled: bool,
        pub needs_ap: u32,
        pub building_cfg: BuildingCfg,
    }
    impl BuildCfg {
        fn with_defaults() -> Self {
            Self {
                enabled: true,
                needs_ap: 3,
                building_cfg: Default::default(),
            }
        }
    }
    impl_default_with!(BuildCfg);
    #[derive(Serialize, Deserialize, Debug)]
    pub struct BuildingCfg {/* TODO：添加建筑设置 */}
    impl BuildingCfg {
        fn with_defaults() -> Self {
            Self {}
        }
    }
    impl_default_with!(BuildingCfg);
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ExploreSettings {
        pub enabled: bool,
        pub items_per_ap: u32,
        pub explore_limits: u32,
    }
    impl ExploreSettings {
        fn with_defaults() -> Self {
            Self {
                enabled: true,
                items_per_ap: 2,
                explore_limits: 0,
            }
        }
    }
    impl_default_with!(ExploreSettings);
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ExchangeSettings {
        pub enabled: bool,
        pub ap_per_food: u32,
        pub exchange_limits: u32,
    }
    impl ExchangeSettings {
        fn with_defaults() -> Self {
            Self {
                enabled: true,
                ap_per_food: 2,
                exchange_limits: 0,
            }
        }
    }
    impl_default_with!(ExchangeSettings);
    /* TODO：添加更多投资设置 */
}
pub mod enums {
    use crate::structs::{GameStateResponse, PlayerInfoResponse};
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Eq, Hash, PartialEq, Copy, Clone, Debug, Deserialize)]
    #[serde(rename_all = "snake_case", untagged)]
    pub enum Items {
        Gold,
        Wood,
        Diamond,
        Ore,
        Food,
        Iron,
    }
    impl TryFrom<&'static str> for Items {
        type Error = String;

        fn try_from(value: &'static str) -> Result<Self, Self::Error> {
            let value = value.to_lowercase();
            if value == "gold" {
                Ok(Self::Gold)
            } else if value == "diamond" {
                Ok(Self::Diamond)
            } else if value == "wood" {
                Ok(Self::Wood)
            } else if value == "ore" {
                Ok(Self::Ore)
            } else if value == "iron" {
                Ok(Self::Iron)
            } else if value == "food" {
                Ok(Self::Food)
            } else {
                Err("No such item".into())
            }
        }
    }
    impl Into<&'static str> for &Items {
        fn into(self) -> &'static str {
            match self {
                Items::Gold => "gold",
                Items::Wood => "wood",
                Items::Diamond => "diamond",
                Items::Ore => "ore",
                Items::Food => "food",
                Items::Iron => "iron",
            }
        }
    }
    #[derive(Eq, PartialEq, Hash, Copy, Clone, Debug, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", untagged)]
    pub enum Building {
        Farm,
        SuperFarm,
        Miner,
        SuperMiner,
        Bank,
        Cannon,
    }
    impl TryFrom<&'static str> for Building {
        type Error = String;

        fn try_from(value: &'static str) -> Result<Self, Self::Error> {
            if value.to_lowercase() == "farm" {
                Ok(Self::Farm)
            } else if value.to_lowercase().contains("farm")
                && value.to_lowercase().contains("super")
            {
                Ok(Self::SuperFarm)
            } else if value == "miner" {
                Ok(Self::Miner)
            } else if value.to_lowercase().contains("miner")
                && value.to_lowercase().contains("super")
            {
                Ok(Self::SuperMiner)
            } else if value == "bank" {
                Ok(Self::Bank)
            } else if value == "cannon" {
                Ok(Self::Cannon)
            } else {
                Err("no such building".into())
            }
        }
    }
    impl Into<&'static str> for &Building {
        fn into(self) -> &'static str {
            match self {
                Building::Farm => "farm",
                Building::SuperFarm => "super_farm",
                Building::Miner => "miner",
                Building::SuperMiner => "super_miner",
                Building::Bank => "bank",
                Building::Cannon => "cannon",
            }
        }
    }
    #[derive(Clone, Deserialize)]
    #[serde(tag = "type", content = "data", rename_all = "snake_case")]
    pub enum PlayerToServerMessage {
        RequestGameState {},
        RequestPlayerInfo { uuid: Uuid },
        SendInvestment { action: InvestmentAction },
    }
    #[derive(Clone, Serialize)]
    #[serde(tag = "type", content = "target", rename_all = "snake_case")]
    pub enum ServerToPlayerMessage {
        Broadcast(ServerBroadcastMessage),
        DataRequired {
            epoch: u32,
            phase: u32,
        },
        GameStateResponse {
            state: GameStateResponse,
        },
        PlayerInfoResponse {
            uuid: Uuid,
            player: PlayerInfoResponse,
        },
        UuidNotice {
            uuid: Uuid,
        },
        InvestmentResult {
            action: InvestmentAction,
            error: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            reason: Option<InvestmentError>,
        },
    }
    #[derive(Serialize, Clone)]
    #[serde(tag = "type", content = "target", rename_all = "snake_case")]
    pub enum InvestmentError {
        ActionPointsDoesNotEnough { need: u32 },
        FoodDoesNotEnough { need: u32 },
        DontHaveMinerOrSuperMiner,
        LimitsExceeded { limit: u32 },
    }
    #[derive(Clone, Serialize)]
    #[serde(tag = "type", content = "target", rename_all = "snake_case")]
    pub enum ServerBroadcastMessage {
        PhaseChanged {
            epoch: u32,
            phase: u32,
        },
        GameStart {},
        HeartBeat {
            state: GameStateResponse,
            interval: u32,
        },
    }
    #[derive(Serialize, Deserialize, Clone)]
    #[serde(tag = "type", content = "data", rename_all = "snake_case")]
    pub enum InvestmentAction {
        Explore {},
        Exchange {},
        Build { building: Building },
        Ore {},
        Pick {},
        Mine {},
        StoreMoney { count: u32 },
        End {},
    }
    #[derive(Clone)]
    pub enum BidAction {
        PlaceBid(u32),
        TakeItem(u32),
        EndTake,
    }
}
pub mod routes {
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
}
pub mod structs {
    use crate::config::GameCfg;
    use crate::enums::{
        Building, Items, PlayerToServerMessage, ServerBroadcastMessage, ServerToPlayerMessage,
    };
    use rand_chacha::ChaCha20Rng;
    use rand_chacha::rand_core::RngCore;
    use serde::Serialize;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::{Mutex, RwLock};
    use tracing::trace;
    use uuid::Uuid;

    #[derive(Serialize, Clone)]
    pub struct GameStateResponse {
        pub players: Vec<Uuid>,
        pub market: Vec<&'static str>,
        pub epoch: u32,
        pub phase: u32,
        pub values: HashMap<&'static str, u32>,
        pub started: bool,
    }
    impl From<&GameState> for GameStateResponse {
        fn from(value: &GameState) -> Self {
            trace!("{:?}", value.market);
            let market = value
                .market
                .iter()
                .map(|x| x.into())
                .collect::<Vec<&'static str>>();
            let epoch = value.epoch;
            let phase = value.phase;
            let started = value.started;
            let values = value
                .resource_values
                .iter()
                .map(|(item, &v)| (item.into(), v))
                .collect::<HashMap<&'static str, u32>>();
            let players = value.players.keys().cloned().collect::<Vec<Uuid>>();
            Self {
                players,
                market,
                epoch,
                phase,
                values,
                started,
            }
        }
    }
    impl GameStateResponse {
        pub fn with_error() -> GameStateResponse {
            Default::default()
        }
    }
    impl Default for GameStateResponse {
        fn default() -> Self {
            GameStateResponse {
                players: Vec::new(),
                market: Vec::new(),
                epoch: 0,
                phase: 0,
                values: HashMap::new(),
                started: false,
            }
        }
    }
    #[derive(Serialize, Clone)]
    pub struct PlayerInfoResponse {
        name: &'static str,
        action_points: u32,
        resources: HashMap<&'static str, u32>,
        buildings: Vec<&'static str>,
        bank_money: u32,
    }
    impl From<&Player> for PlayerInfoResponse {
        fn from(value: &Player) -> Self {
            let action_points = value.action_points;
            let resources = value
                .resources
                .iter()
                .map(|(x, y)| {
                    let item_str: &'static str = x.into();
                    (item_str, *y)
                })
                .collect::<HashMap<&'static str, u32>>();
            let buildings = value
                .buildings
                .iter()
                .map(|x| x.into())
                .collect::<Vec<&'static str>>();
            let bank_money = value.bank_money;
            Self {
                name: value.name,
                action_points,
                resources,
                buildings,
                bank_money,
            }
        }
    }
    impl PlayerInfoResponse {
        pub fn with_error() -> PlayerInfoResponse {
            Default::default()
        }
    }
    impl Default for PlayerInfoResponse {
        fn default() -> Self {
            PlayerInfoResponse {
                name: "",
                action_points: 0,
                resources: HashMap::new(),
                buildings: Vec::new(),
                bank_money: 0,
            }
        }
    }
    pub struct AppState {
        pub cfg: Arc<GameCfg>,
        pub game_state: Arc<RwLock<GameState>>,
        pub rng: ChaCha20Rng,
    }
    impl AppState {
        pub fn new(cfg: GameCfg, game_state: GameState, rng: ChaCha20Rng) -> AppState {
            Self {
                cfg: Arc::new(cfg),
                game_state: Arc::new(RwLock::new(game_state)),
                rng,
            }
        }
    }
    pub struct Channel<T> {
        pub sender: tokio::sync::mpsc::Sender<T>,
        pub receiver: Arc<Mutex<tokio::sync::mpsc::Receiver<T>>>,
    }
    impl<T> Channel<T> {
        pub fn new() -> Channel<T> {
            let (s, r) = tokio::sync::mpsc::channel(255);
            Channel {
                sender: s,
                receiver: Arc::new(Mutex::new(r)),
            }
        }
    }
    pub struct Player {
        pub name: &'static str,
        pub resources: HashMap<Items, u32>,
        pub action_points: u32,
        pub buildings: Vec<Building>,
        pub bank_money: u32,
        pub from_channel: Channel<PlayerToServerMessage>,
        pub to_channel: Channel<ServerToPlayerMessage>,
    }
    impl Player {
        pub fn new(player_name: &'static str) -> Player {
            let mut res = Self {
                name: player_name,
                resources: HashMap::new(),
                action_points: 0,
                buildings: Vec::new(),
                bank_money: 0,
                from_channel: Channel::new(),
                to_channel: Channel::new(),
            };
            res.resources.insert(Items::Gold, 0);
            res.resources.insert(Items::Iron, 0);
            res.resources.insert(Items::Wood, 0);
            res.resources.insert(Items::Diamond, 0);
            res.resources.insert(Items::Ore, 0);
            res.resources.insert(Items::Food, 0);
            res
        }
        pub fn with_cfg(player_name: &'static str, cfg: &GameCfg) -> Player {
            let mut res = Self::new(player_name);
            cfg.game_rules
                .prepare
                .defaults_give_player
                .apply_to_player(&mut res);
            res
        }
    }
    pub struct GameState {
        pub players: HashMap<Uuid, Player>,
        pub market: Vec<Items>,
        pub current_deck: Vec<Items>,
        pub epoch: u32,
        pub phase: u32,
        pub resource_values: HashMap<Items, u32>,
        pub started: bool,
    }
    impl GameState {
        pub fn new() -> GameState {
            let mut res = GameState {
                players: HashMap::new(),
                market: Vec::new(),
                current_deck: Vec::new(),
                epoch: 1,
                phase: 1,
                resource_values: HashMap::new(),
                started: false,
            };
            res.resource_values.insert(Items::Diamond, 8);
            res.resource_values.insert(Items::Gold, 6);
            res.resource_values.insert(Items::Wood, 2);
            res.resource_values.insert(Items::Ore, 3);
            res.resource_values.insert(Items::Food, 1);
            res.resource_values.insert(Items::Iron, 2);
            res
        }
        async fn apply_configurations(&mut self, conf: &GameCfg) {
            self.resource_values = (&conf.game_rules.resource_values_default.clone()).into();
        }
        pub async fn initialize(&mut self, conf: &GameCfg, rng: &mut ChaCha20Rng) {
            self.apply_configurations(conf).await;
            let deck: HashMap<Items, u32> = (&conf.game_rules.prepare.deck).into();
            deck.iter().for_each(|(x, y)| {
                for _ in 0..*y {
                    self.current_deck.push(x.clone())
                }
            });
            let sze = self.current_deck.len();
            for i in 0..sze {
                let index = rng.next_u32() % (sze as u32);
                let tmp = self.current_deck[i];
                self.current_deck[i] = self.current_deck[index as usize];
                self.current_deck[index as usize] = tmp;
            }
            let mut cards: Vec<Items> = self
                .current_deck
                .drain(0..conf.game_rules.prepare.draw_cards as usize)
                .collect();
            self.market.append(&mut cards);
        }
        pub async fn broadcast(&self, message: ServerBroadcastMessage) {
            for (_, player) in self.players.iter() {
                let _ = player
                    .to_channel
                    .sender
                    .send(ServerToPlayerMessage::Broadcast { 0: message.clone() })
                    .await;
            }
        }
        pub async fn send_to(&self, uuid: &Uuid, message: ServerToPlayerMessage) {
            self.players
                .get(uuid)
                .unwrap()
                .to_channel
                .sender
                .send(message)
                .await
                .unwrap();
        }
        pub async fn register_player(&mut self, player: Player) -> Uuid {
            loop {
                let uuid = Uuid::new_v4();
                if !self.players.contains_key(&uuid) {
                    self.players.insert(uuid.clone(), player);
                    break uuid;
                }
            }
        }
        pub async fn unregister_player(&mut self, uuid: &Uuid) {
            if let Some(player) = self.players.remove(uuid) {
                drop(player);
            }
        }
        pub async fn increase_phase(&mut self) {
            self.phase += 1;
            if self.phase == 5 {
                self.epoch += 1;
                self.phase = 0;
            }
        }
    }
    impl Default for GameState {
        fn default() -> Self {
            Self::new()
        }
    }
}
