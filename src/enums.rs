use crate::structs::{GameStateResponse, PlayerInfoResponse};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use uuid::Uuid;

#[derive(Eq, Hash, PartialEq, Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
#[serde(rename_all = "snake_case")]
pub enum Building {
    Farm,
    SuperFarm,
    Miner,
    SuperMiner,
    Bank,
    Cannon,
    Pickaxe,
    Lumber,
}
impl TryFrom<&'static str> for Building {
    type Error = String;

    fn try_from(value: &'static str) -> Result<Self, Self::Error> {
        let value = value.to_lowercase();
        if value == "farm" {
            Ok(Self::Farm)
        } else if value.contains("farm") && value.contains("super") {
            Ok(Self::SuperFarm)
        } else if value == "miner" {
            Ok(Self::Miner)
        } else if value.contains("miner") && value.contains("super") {
            Ok(Self::SuperMiner)
        } else if value == "bank" {
            Ok(Self::Bank)
        } else if value == "cannon" {
            Ok(Self::Cannon)
        } else if value == "pickaxe" {
            Ok(Self::Pickaxe)
        } else if value == "lumber" {
            Ok(Self::Lumber)
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
            Building::Pickaxe => "pickaxe",
            Building::Lumber => "lumber",
        }
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Events {
    PirateAttack,
    CropBonus,
    ApBonus,
    Famine,
}
#[derive(Clone, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum PlayerToServerMessage {
    RequestGameState {},
    RequestPlayerInfo { uuid: Uuid },
    SendInvestment { action: InvestmentAction },
    SendBidding { bidding: u32 },
    SendContending { action: ContendingAction },
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
    BuildingWorked {
        building: Building,
    },
    BiddingResult {
        bidding: u32,
        error: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<BiddingError>,
    },
    ContendingResult {
        action: ContendingAction,
        error: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<ContendingError>,
    },
}
#[derive(Serialize, Clone)]
#[serde(tag = "type", content = "target", rename_all = "snake_case")]
pub enum InvestmentError {
    NoEnoughActionPoints {
        need: u32,
    },
    NoEnoughFood {
        need: u32,
    },
    DontHaveMinerOrSuperMiner {},
    LimitsExceeded {
        limit: u32,
    },
    ActionIsNotEnabled {},
    BuildingIsNotEnabled {},
    NoEnoughMaterials {
        need_items: HashMap<Items, u32>,
        need_buildings: HashMap<Building, u32>,
    },
    NoEnoughOre {
        need: u32,
    },
    NoEnoughItem {
        need: HashMap<Items, u32>,
    },
}
#[derive(Serialize, Clone)]
#[serde(tag = "type", content = "target", rename_all = "snake_case")]
pub enum BiddingError {
    NoEnoughActionPoints { need: u32 },
    BiddingNotValid { max: u32, min: u32 },
}
#[derive(Serialize, Clone)]
#[serde(tag = "type", content = "target", rename_all = "snake_case")]
pub enum ContendingError {
    NoEnoughActionPoints { need: u32 },
    ItemNotFound {},
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
    MarketEmpty {},
    GameOver {
        player_total_value: HashMap<Uuid, u32>,
    },
    BiddingSorted {
        order: HashSet<Uuid>,
    },
    OthersBidding {
        uuid: Uuid,
        bidding: u32,
    },
    OthersContending {
        action: ContendingAction,
    },
    ValueChanged {
        item: Items,
        now: u32,
    },
    EventChosen {
        event: Events,
    },
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum InvestmentAction {
    Explore {},
    Exchange {},
    Build { building: Building },
    CrushOre {},
    StoreMoney { item: Items, count: u32 },
    End {},
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ContendingAction {
    Take { index: usize, item: Items },
    End {},
}
