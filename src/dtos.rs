use std::collections::{HashMap, HashSet};
use serde::Serialize;
use tracing::trace;
use crate::{GameState, Player};

#[derive(Serialize)]
pub struct GameStateResponse {
    pub players: Vec<&'static str>,
    pub market: Vec<&'static str>,
    pub epoch: u32,
    pub phase: u32,
    pub values: HashMap<&'static str, u32>,
    pub started: bool
}
impl From<&GameState> for GameStateResponse {
    fn from(value: &GameState) -> Self {
        trace!("{:?}", value.market);
        let market = value.market.iter()
            .map(|x| x.into()).collect::<Vec<&'static str>>();
        let epoch = value.epoch;
        let phase = value.phase;
        let started = value.started;
        let values = value.resource_values.iter()
            .map(|(item, &v)| (item.into(), v)).collect::<HashMap<&'static str, u32>>();
        let players = value.players.keys()
            .cloned()
            .collect::<Vec<&'static str>>();
        Self {
            players,
            market,
            epoch,
            phase,
            values,
            started
        }
    }
}
impl GameStateResponse {
    pub fn with_error() -> GameStateResponse {
        GameStateResponse {
            players: Vec::new(),
            market: Vec::new(),
            epoch: 0,
            phase: 0,
            values: HashMap::new(),
            started: false
        }
    }
}
#[derive(Serialize)]
pub struct PlayerInfoResponse {
    action_points: u32,
    resources: HashMap<&'static str, u32>,
    buildings: Vec<&'static str>,
    bank_money: u32,
}
impl From<&Player> for PlayerInfoResponse {
    fn from(value: &Player) -> Self {
        let action_points = value.action_points;
        let resources = value.resources.iter()
            .map(|(x, y)| {
                let item_str: &'static str = x.into();
                (item_str, *y)
            }).collect::<HashMap<&'static str, u32>>();
        let buildings = value.buildings.iter()
            .map(|x| x.into())
            .collect::<Vec<&'static str>>();
        let bank_money = value.bank_money;
        Self {
            action_points,
            resources,
            buildings,
            bank_money
        }
    }
}
impl PlayerInfoResponse {
    pub fn with_error() -> PlayerInfoResponse {
        PlayerInfoResponse {
            action_points: 0,
            resources: HashMap::new(),
            buildings: Vec::new(),
            bank_money: 0
        }
    }
}