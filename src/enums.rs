use crate::NoSuchFound;
use crate::structs::{GameStateResponse, PlayerInfoResponse};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use uuid::Uuid;

#[derive(Eq, Hash, PartialEq, Copy, Clone, Debug)]
pub enum Items {
    Gold,
    Wood,
    Diamond,
    Ore,
    Food,
    Iron,
}
impl TryFrom<&'static str> for Items {
    type Error = NoSuchFound;

    fn try_from(value: &'static str) -> Result<Self, Self::Error> {
        if value == "金币" {
            Ok(Self::Gold)
        } else if value == "钻石" {
            Ok(Self::Diamond)
        } else if value == "木材" {
            Ok(Self::Wood)
        } else if value == "矿石" {
            Ok(Self::Ore)
        } else if value == "铁" {
            Ok(Self::Iron)
        } else if value == "食物" {
            Ok(Self::Food)
        } else {
            Err(NoSuchFound::NoSuchItems(value))
        }
    }
}
impl Into<&'static str> for &Items {
    fn into(self) -> &'static str {
        match self {
            Items::Gold => "金币",
            Items::Wood => "木材",
            Items::Diamond => "钻石",
            Items::Ore => "矿石",
            Items::Food => "食物",
            Items::Iron => "铁",
        }
    }
}
#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug)]
pub enum Building {
    Farm,
    SuperFarm,
    Miner,
    SuperMiner,
    Bank,
    Cannon,
}
impl TryFrom<&'static str> for Building {
    type Error = NoSuchFound;

    fn try_from(value: &'static str) -> Result<Self, Self::Error> {
        if value == "农场" {
            Ok(Self::Farm)
        } else if value.contains("农场") && value != "农场" {
            Ok(Self::SuperFarm)
        } else if value == "矿机" {
            Ok(Self::Miner)
        } else if value.contains("矿机") && value != "矿机" {
            Ok(Self::SuperMiner)
        } else if value == "银行" {
            Ok(Self::Bank)
        } else if value == "炮台" {
            Ok(Self::Cannon)
        } else {
            Err(NoSuchFound::NoSuchBuildings(value))
        }
    }
}
impl Into<&'static str> for &Building {
    fn into(self) -> &'static str {
        match self {
            Building::Farm => "农场",
            Building::SuperFarm => "无敌农场",
            Building::Miner => "矿机",
            Building::SuperMiner => "高级矿机",
            Building::Bank => "银行",
            Building::Cannon => "炮台",
        }
    }
}
#[derive(Clone, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum PlayerToServerMessage {
    #[serde(rename = "request_game_state")]
    RequestGameState {},
    #[serde(rename = "request_player_info")]
    RequestPlayerInfo { uuid: Uuid },
    #[serde(rename = "send_investment")]
    SendInvestment {},
}
#[derive(Clone, Serialize)]
#[serde(tag = "type", content = "target")]
pub enum ServerToPlayerMessage {
    #[serde(rename = "broadcast", serialize_with = "serialize_change")]
    Broadcast { raw: ServerBroadcastMessage },
    #[serde(rename = "data_required")]
    DataRequired { epoch: u32, phase: u32 },
    #[serde(rename = "game_state_response")]
    GameStateResponse { state: GameStateResponse },
    #[serde(rename = "player_info")]
    PlayerInfoResponse {
        uuid: Uuid,
        player: PlayerInfoResponse,
    },
    #[serde(rename = "uuid_notice")]
    UuidNotice { uuid: Uuid },
}
fn serialize_change<T, S>(raw: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: Serialize,
    S: serde::Serializer,
{
    raw.serialize(serializer)
}
#[derive(Clone, Serialize)]
#[serde(tag = "type", content = "target")]
pub enum ServerBroadcastMessage {
    #[serde(rename = "phase_changed")]
    PhaseChanged { epoch: u32, phase: u32 },
    #[serde(rename = "game_start")]
    GameStart {},
    #[serde(rename = "heartbeat")]
    HeartBeat {
        state: GameStateResponse,
        interval: u32,
    },
}
#[derive(Clone)]
pub enum InvestmentAction {
    Explore,
    Exchange,
    Build(Building),
    Ore,
    Pick,
    Mine,
    Bank(u32),
    End,
}
#[derive(Clone)]
pub enum BidAction {
    PlaceBid(u32),
    TakeItem(u32),
    EndTake,
}
