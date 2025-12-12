pub mod config;
pub mod dtos;
pub mod enums;
mod game;

use crate::config::GameCfg;
use crate::enums::{Buildings, Items};
use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;
// Functions

// Enums
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
}
impl AppState {
    pub fn new(cfg: GameCfg, game_state: GameState) -> AppState {
        Self {
            cfg: Arc::new(Mutex::new(cfg)),
            game_state: Arc::new(RwLock::new(game_state))
        }
    }
}
pub struct Player {
    resources: HashMap<Items, u32>,
    action_points: u32,
    buildings: HashSet<Buildings>,
    bank_money: u32,
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
    pub market: HashSet<Items>,
    pub current_deck: HashSet<Items>,
    pub epoch: u32,
    pub phase: u32,
    pub resource_values: HashMap<Items, u32>,
    pub started: bool
}
impl GameState {
    pub fn new() -> GameState {
        let mut res = GameState {
            players: HashMap::new(),
            market: HashSet::new(),
            current_deck: HashSet::new(),
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
    pub fn apply_configurations(&mut self, conf: &GameCfg) {
        self.resource_values = (&conf.game_rules.resource_values_default.clone()).into();
    }
}

// DTOs

// Routes