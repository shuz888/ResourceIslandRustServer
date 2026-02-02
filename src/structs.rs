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
