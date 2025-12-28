use crate::NoSuchFound;
use serde::Serialize;
use std::convert::TryFrom;

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
#[derive(Clone)]
pub enum PlayerToServerMessage {
    Investment { action: InvestmentAction },
    Bid { action: BidAction },
}
#[derive(Clone, Serialize)]
#[serde(tag = "type", content = "target")]
#[serde(rename_all = "lowercase")]
#[serde(rename_all_fields = "lowercase")]
pub enum ServerToPlayerMessage {
    #[serde(serialize_with = "serialize_stp_broadcast")]
    Broadcast { raw: ServerBroadcastMessage },
}
fn serialize_stp_broadcast<S>(
    raw: &ServerBroadcastMessage,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    raw.serialize(serializer)
}
#[derive(Clone, Serialize)]
#[serde(tag = "type", content = "target")]
#[serde(rename_all = "lowercase")]
#[serde(rename_all_fields = "lowercase")]
pub enum ServerBroadcastMessage {
    PhaseChanged { epoch: u32, phase: u32 },
    DataRequired { epoch: u32, phase: u32 },
    GameStart,
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
