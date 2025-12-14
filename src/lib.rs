pub mod config;
pub mod dtos;
pub mod enums;
mod game;

use crate::config::GameCfg;
use crate::enums::{Building, Items, ServerBroadcastMessage, ServerToPlayerMessage};
use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use rand::prelude::SliceRandom;
use rand::Rng;
use thiserror::Error;
#[derive(Error, Debug)]
pub enum NoSuchFound {
    #[error("你传入的{0}物品无法找到对应枚举量")]
    NoSuchItems(&'static str),
    #[error("你传入的{0}建筑无法找到对应枚举量")]
    NoSuchBuildings(&'static str)
}

// Structs
pub struct AppState {
    pub cfg: Arc<Mutex<GameCfg>>,
    pub game_state: Arc<RwLock<GameState>>,
    pub channels: Channels,
}
impl AppState {
    pub fn new(cfg: GameCfg, game_state: GameState) -> AppState {
        Self {
            cfg: Arc::new(Mutex::new(cfg)),
            game_state: Arc::new(RwLock::new(game_state)),
            channels: Channels::new(),
        }
    }
}
pub struct Channels {
    pub server_to_player: Channel<ServerToPlayerMessage>,
    pub server_broadcast: Channel<ServerBroadcastMessage>,
}
impl Channels {
    pub fn new() -> Channels {
        Channels {
            server_to_player: Channel::new(),
            server_broadcast: Channel::new(),
        }
    }
}
pub struct Channel<T> {
    pub sender: crossbeam_channel::Sender<T>,
    pub receiver: crossbeam_channel::Receiver<T>,
}
impl<T> Channel<T> {
    pub fn new() -> Channel<T> {
        let (s, r) = crossbeam_channel::bounded(250);
        Channel {
            sender: s,
            receiver: r,
        }
    }
}
pub struct Player {
    pub resources: HashMap<Items, u32>,
    pub action_points: u32,
    pub buildings: HashSet<Building>,
    pub bank_money: u32,
}
impl Player {
    pub fn new() -> Player{
        let mut res = Self {
            resources: HashMap::new(),
            action_points: 0,
            buildings: HashSet::new(),
            bank_money: 0,
        };
        res.resources.insert(Items::Gold, 0);
        res.resources.insert(Items::Iron, 0);
        res.resources.insert(Items::Wood, 0);
        res.resources.insert(Items::Diamond, 0);
        res.resources.insert(Items::Ore, 0);
        res.resources.insert(Items::Food, 0);
        res
    }
}
pub struct GameState {
    pub players: HashMap<&'static str, Player>,
    pub market: Vec<Items>,
    pub current_deck: Vec<Items>,
    pub epoch: u32,
    pub phase: u32,
    pub resource_values: HashMap<Items, u32>,
    pub started: bool
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
            started: true
        };
        res.resource_values.insert(Items::Diamond, 8);
        res.resource_values.insert(Items::Gold, 6);
        res.resource_values.insert(Items::Wood, 2);
        res.resource_values.insert(Items::Ore, 3);
        res.resource_values.insert(Items::Food, 1);
        res.resource_values.insert(Items::Iron, 2);
        res
    }
    fn apply_configurations(&mut self, conf: &GameCfg) {
        self.resource_values = (&conf.game_rules.resource_values_default.clone()).into();
    }
    pub fn initialize(&mut self, conf: &GameCfg) {
        self.apply_configurations(conf);
        let deck: HashMap<Items, u32> = (&conf.game_rules.prepare.deck).into();
        deck.iter().for_each(|(x, y)| {
            for _ in 0..*y {
                self.current_deck.push(x.clone())
            }
        });
        {
            let mut rng = rand::rng();
            self.current_deck.shuffle(&mut rng);
            let mut cards: Vec<Items> = self.current_deck.drain(0..conf.game_rules.prepare.draw_cards as usize).collect();
            self.market.append(&mut cards);
        }
    }
}